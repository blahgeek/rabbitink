use log::{debug, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::imgproc::dithering;

use super::driver::it8915::{DisplayMode, MemMode, IT8915};
use super::image::*;
use super::imgproc::{MonoImgproc, MonoImgprocOptions};
use super::run_mode::RunMode;
use super::source::Source;

type RowSet = std::collections::BTreeSet<i32>;

fn compute_modified_row_range<const BPP: i32>(
    m_a: &impl ConstImage<BPP>,
    m_b: &impl ConstImage<BPP>,
) -> RowSet {
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

pub struct AppOptions {
    pub reload_flag: Arc<AtomicBool>,
    pub terminate_flag: Arc<AtomicBool>,
    pub run_mode_config_path: std::path::PathBuf,
}

pub struct App<S> {
    driver: IT8915,
    source: S,
    options: AppOptions,
    current_run_mode: RunMode,

    mono_imgproc: Option<MonoImgproc>, // initialize on first frame, for correct pitch
    mono_loaded_frame: Option<ImageBuffer<1>>,
    gray_loaded_frame: Option<ImageBuffer<8>>,

    dirty_rows: RowSet,
    displaying_rows: RowSet,
    full_refreshed: bool,
}

const DRIVER_POLL_READY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);
const SOURCE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

const FULL_REFRESH_IDLE_DELAY: std::time::Duration = std::time::Duration::from_secs(120);
const FULL_REFRESH_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3); // prevent duplicated full refresh request within this period, if nothing is changed
const TEXT_ROW_TYPICAL_HEIGHT: i32 = 40; // when considering "row ratio" below, "expand" each pixel row to this height,
                                         // so that the "row ratio" is more close to what we assume
const SLOW_REFRESH_ROW_RATIO_THRESHOLD: f32 = 0.5; // do a slow (e.g. DU instead of A2) refresh if more than this ratio of rows are changed

impl<S> App<S>
where
    S: Source,
{
    pub fn new(driver: IT8915, source: S, options: AppOptions) -> App<S> {
        let current_run_mode =
            RunMode::read_from_file(&options.run_mode_config_path).unwrap_or_default();
        App {
            driver,
            source,
            options,
            current_run_mode,
            mono_imgproc: None,
            mono_loaded_frame: None,
            gray_loaded_frame: None,
            dirty_rows: RowSet::default(),
            displaying_rows: RowSet::default(),
            full_refreshed: false,
        }
    }

    // get frame and load into driver,modify display_dirty_range
    // return true if it's modified (display required)
    fn load_frame_mono(&mut self) -> anyhow::Result<()> {
        let screen_size = self.driver.get_screen_size();
        let t_load_start = std::time::Instant::now();

        let bgra_img = self.source.get_frame()?;
        assert_eq!(bgra_img.size(), screen_size);
        let t_got_frame = std::time::Instant::now();

        let mut new_frame = ImageBuffer::<1>::new(
            screen_size.width,
            screen_size.height,
            Some(self.driver.get_mem_pitch(MemMode::Mem1bpp)),
        );
        if self.mono_imgproc.is_none() {
            self.mono_imgproc = Some(pollster::block_on(MonoImgproc::new(MonoImgprocOptions {
                image_size: screen_size,
                bgra_pitch: bgra_img.pitch(),
                bw_pitch: self.driver.get_mem_pitch(MemMode::Mem1bpp),
            })));
        }
        let dithering_method = match self.current_run_mode {
            RunMode::Mono(v) => v,
            RunMode::Gray => unreachable!(),
        };
        self.mono_imgproc
            .as_mut()
            .unwrap()
            .process(&bgra_img, &mut new_frame, dithering_method);
        let t_imgproc = std::time::Instant::now();

        let mut modified_range = match &self.mono_loaded_frame {
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
                .load_image_fullwidth_1bpp(*modified_range.first().unwrap() as u32, &load_subimg)?;
            self.dirty_rows.append(&mut modified_range);
            drop(modified_range);
            self.mono_loaded_frame = Some(new_frame);
        }
        let t_loaded = std::time::Instant::now();

        debug!("New mono frame loaded, {} rows dirty accumulated. Cost: get frame: {:?}, imgproc: {:?}, load: {:?}",
               self.dirty_rows.len(),
               t_got_frame - t_load_start,
               t_imgproc - t_got_frame,
               t_loaded - t_imgproc);
        Ok(())
    }

    fn load_frame_gray(&mut self) -> anyhow::Result<()> {
        let screen_size = self.driver.get_screen_size();
        let bgra_img = self.source.get_frame()?;
        assert_eq!(bgra_img.size(), screen_size);
        let gray_img = dithering::floyd_steinberg(&bgra_img, dithering::GREY16_TARGET_COLOR_SPACE);

        // let mut modified = true;
        if let Some(loaded_frame) = &self.gray_loaded_frame {
            if compute_modified_row_range(loaded_frame, &gray_img).is_empty() {
                return Ok(());
            }
        }
        // TODO: load only modified rows
        self.driver.load_image_fullwidth_8bpp(0, &gray_img)?;
        self.gray_loaded_frame = Some(gray_img);

        self.dirty_rows = RowSet::from_iter(0..screen_size.height);
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
            < (screen_size.height as f32 * SLOW_REFRESH_ROW_RATIO_THRESHOLD) as i32
        {
            self.current_run_mode.display_mode_fast()
        } else {
            self.current_run_mode.display_mode_slow()
        };
        let dirty_start = *self.dirty_rows.first().unwrap();
        let dirty_end = *self.dirty_rows.last().unwrap() + 1;
        self.driver.display_area(
            (0, dirty_start).into(),
            (screen_size.width, dirty_end - dirty_start).into(),
            mode,
            self.current_run_mode.mem_mode(),
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
            self.current_run_mode.mem_mode(),
            true,
        )?;
        self.dirty_rows.clear();
        self.displaying_rows.clear();
        self.full_refreshed = true;
        Ok(())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut t_last_update = std::time::Instant::now();
        while !self.options.terminate_flag.swap(false, Ordering::Relaxed) {
            let reload_requested = self.options.reload_flag.swap(false, Ordering::Relaxed);
            if reload_requested {
                let new_run_mode =
                    RunMode::read_from_file(&self.options.run_mode_config_path).unwrap_or_default();
                if new_run_mode != self.current_run_mode {
                    info!("Switching to new run mode: {:?}", new_run_mode);
                    self.poll_display_ready(/* block */ true)?;
                    self.driver.reset_display()?;
                    self.mono_loaded_frame = None;
                    self.gray_loaded_frame = None;
                    self.current_run_mode = new_run_mode;
                }
            }

            match self.current_run_mode {
                RunMode::Mono(_) => self.load_frame_mono()?,
                RunMode::Gray => self.load_frame_gray()?,
            }
            let need_display = !self.dirty_rows.is_empty();

            let full_refresh = (reload_requested
                && (!self.full_refreshed || t_last_update.elapsed() > FULL_REFRESH_MIN_INTERVAL))
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
