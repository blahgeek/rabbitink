use anyhow::bail;
use log::{debug, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::driver::it8915::{DisplayMode, MonoDriver};
use super::image::*;
use super::imgproc::{DitheringMethod, Imgproc, ImgprocOptions};
use super::source::Source;

type RowSet = std::collections::BTreeSet<i32>;

fn compute_modified_row_range(m_a: &impl ConstImage<1>, m_b: &impl ConstImage<1>) -> RowSet {
    assert_eq!(m_a.size(), m_b.size());
    assert_eq!(m_a.pitch(), m_b.pitch());

    let modified_rows = (0..m_a.height()).filter(|y| {
        let y = *y;
        let ptr_a = m_a.ptr(y);
        let ptr_b = m_b.ptr(y);
        let cmp_res = unsafe {
            libc::memcmp(
                ptr_a as *const libc::c_void,
                ptr_b as *const libc::c_void,
                m_a.pitch() as usize,
            )
        };
        return cmp_res != 0;
    });
    RowSet::from_iter(modified_rows)
}

pub struct ControlOptions {
    pub full_refresh_flag: Arc<AtomicBool>,
    pub terminate_flag: Arc<AtomicBool>,
}

pub struct Controller<S> {
    driver: MonoDriver,
    source: S,
    options: ControlOptions,

    imgproc: Option<Imgproc>, // initialize on first frame, for correct pitch

    loaded_frame: Option<ImageBuffer<1>>,

    dirty_rows: RowSet,
    displaying_rows: RowSet,
    full_refreshed: bool,
}

const DRIVER_POLL_READY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);
const SOURCE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

const FULL_REFRESH_IDLE_DELAY: std::time::Duration = std::time::Duration::from_secs(120);
const TEXT_ROW_TYPICAL_HEIGHT: i32 = 40; // when considering "row ratio" below, "expand" each pixel row to this height,
                                         // so that the "row ratio" is more close to what we assume
const DU_REFRESH_ROW_RATIO_THRESHOLD: f32 = 0.5; // do a DU (instead of A2) refresh if more than this ratio of rows are changed

impl<S> Controller<S>
where
    S: Source,
{
    pub fn new(driver: MonoDriver, source: S, options: ControlOptions) -> Controller<S> {
        Controller {
            driver,
            source,
            options,
            imgproc: None,
            loaded_frame: None,
            dirty_rows: RowSet::default(),
            displaying_rows: RowSet::default(),
            full_refreshed: false,
        }
    }

    // get frame and load into driver,modify display_dirty_range
    // return true if it's modified (display required)
    fn load_frame(&mut self) -> anyhow::Result<()> {
        let screen_size = self.driver.get_screen_size();
        let t_load_start = std::time::Instant::now();

        let bgra_img = self.source.get_frame()?;
        if bgra_img.size() != screen_size {
            bail!(
                "Source returned invalid sized frame: {:?}, screen size {:?}",
                bgra_img.size(),
                screen_size
            );
        }
        let t_got_frame = std::time::Instant::now();

        let mut new_frame = ImageBuffer::<1>::new(
            screen_size.width,
            screen_size.height,
            Some(self.driver.get_mem_pitch()),
        );
        if self.imgproc.is_none() {
            self.imgproc = Some(pollster::block_on(Imgproc::new(ImgprocOptions {
                image_size: screen_size,
                bgra_pitch: bgra_img.pitch(),
                bw_pitch: self.driver.get_mem_pitch(),
                dithering_method: DitheringMethod::Bayers4,
            })));
        }
        self.imgproc
            .as_ref()
            .unwrap()
            .process(&bgra_img, &mut new_frame);
        let t_imgproc = std::time::Instant::now();

        let mut modified_range = match &self.loaded_frame {
            None => RowSet::from_iter(0..screen_size.height),
            Some(loaded_frame) => compute_modified_row_range(&new_frame, loaded_frame),
        };
        if !modified_range.is_empty() {
            let load_subimg = new_frame.subimg(
                (0, *modified_range.first().unwrap()).into(),
                (
                    screen_size.width,
                    *modified_range.last().unwrap() - modified_range.first().unwrap() + 1,
                )
                    .into(),
            );
            self.driver
                .load_image_fullwidth(*modified_range.first().unwrap() as u32, &load_subimg)?;
            self.dirty_rows.append(&mut modified_range);
            drop(modified_range);
            self.loaded_frame = Some(new_frame);
        }
        let t_loaded = std::time::Instant::now();

        debug!("New frame loaded, {} rows dirty accumulated. Cost: get frame: {:?}, imgproc: {:?}, load: {:?}",
               self.dirty_rows.len(),
               t_got_frame - t_load_start,
               t_imgproc - t_got_frame,
               t_loaded - t_imgproc);
        Ok(())
    }

    // return if current display dirty range is actually not blocked by displaying rows
    // NOTE that we only display one full range
    fn can_display_nonoverlapping(&self) -> bool {
        if self.dirty_rows.is_empty() || self.displaying_rows.is_empty() {
            return false;
        }
        let dirty_start = *self.dirty_rows.first().unwrap();
        let dirty_end = *self.dirty_rows.last().unwrap() + 1;
        self.displaying_rows
            .iter()
            .all(|x| *x < dirty_start || *x >= dirty_end)
    }

    fn poll_display_ready(&mut self, block: bool) -> anyhow::Result<bool> {
        while self.driver.read_busy_state()? {
            if !block {
                return Ok(false);
            }
            std::thread::sleep(DRIVER_POLL_READY_INTERVAL);
        }
        self.displaying_rows.clear();
        return Ok(true);
    }

    fn do_display_nonblock(&mut self) -> anyhow::Result<DisplayMode> {
        assert!(!self.dirty_rows.is_empty());
        let screen_size = self.driver.get_screen_size();
        let num_dirty_rows_expanded = self
            .dirty_rows
            .iter()
            .map(|x| *x / TEXT_ROW_TYPICAL_HEIGHT)
            .collect::<std::collections::BTreeSet<i32>>()
            .len() as i32
            * TEXT_ROW_TYPICAL_HEIGHT;
        let mode = if num_dirty_rows_expanded
            < (screen_size.height as f32 * DU_REFRESH_ROW_RATIO_THRESHOLD) as i32
        {
            DisplayMode::A2
        } else {
            DisplayMode::DU
        };
        let dirty_start = *self.dirty_rows.first().unwrap();
        let dirty_end = *self.dirty_rows.last().unwrap() + 1;
        self.driver.display_area(
            (0, dirty_start).into(),
            (screen_size.width, dirty_end - dirty_start).into(),
            mode,
            false,
        )?;
        self.displaying_rows.extend(dirty_start..dirty_end);
        self.dirty_rows.clear();
        self.full_refreshed = false;
        Ok(mode)
    }

    fn do_display_full_refresh_block(&mut self) -> anyhow::Result<()> {
        self.driver.display_area(
            (0, 0).into(),
            self.driver.get_screen_size(),
            DisplayMode::GC16,
            true,
        )?;
        self.dirty_rows.clear();
        self.displaying_rows.clear();
        self.full_refreshed = true;
        Ok(())
    }

    pub fn run_loop(&mut self) -> anyhow::Result<()> {
        let mut t_last_update = std::time::Instant::now();
        while !self.options.terminate_flag.swap(false, Ordering::Relaxed) {
            self.load_frame()?;
            let need_display = !self.dirty_rows.is_empty();

            let full_refresh = self
                .options
                .full_refresh_flag
                .swap(false, Ordering::Relaxed)
                || (!need_display
                    && t_last_update.elapsed() > FULL_REFRESH_IDLE_DELAY
                    && !self.full_refreshed);
            if full_refresh {
                info!("Full refresh!");
                self.poll_display_ready(/* block */ true)?;
                self.do_display_full_refresh_block()?;
                t_last_update = std::time::Instant::now();
                continue;
            }

            if !need_display {
                // frame not changed
                std::thread::sleep(SOURCE_POLL_INTERVAL);
                continue;
            }

            if !self.poll_display_ready(/* block */ false)? && !self.can_display_nonoverlapping() {
                // cannot display now. we would wait for ready and loop again to get the newest frame
                self.poll_display_ready(/* block */ true)?;
                continue;
            }

            let displayed_mode = self.do_display_nonblock()?;

            let t_update = std::time::Instant::now();
            info!(
                "New frame displayed, interval: {:?}, mode: {:?}",
                t_update - t_last_update,
                displayed_mode
            );
            t_last_update = t_update;
        }
        self.driver.reset_display()?;
        Ok(())
    }
}
