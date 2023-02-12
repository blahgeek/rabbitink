use anyhow::bail;
use log::{debug, info};

use super::driver::it8915::{DisplayMode, MonoDriver};
use super::image::*;
use super::imgproc::{Imgproc, ImgprocOptions};
use super::source::Source;

fn compute_modified_row_range(
    m_a: &impl ConstImage<1>,
    m_b: &impl ConstImage<1>,
) -> Option<(i32, i32)> {
    assert_eq!(m_a.size(), m_b.size());
    assert_eq!(m_a.pitch(), m_b.pitch());
    let row_modified_status: Vec<(usize, bool)> = (0..m_a.height())
        .map(|y| {
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
        })
        .enumerate()
        .collect();

    let start = row_modified_status.iter().find(|x| x.1).map(|x| x.0 as i32);
    if let Some(start) = start {
        let end = row_modified_status
            .iter()
            .rfind(|x| x.1)
            .map(|x| 1 + x.0 as i32)
            .unwrap();
        Some((start, end))
    } else {
        None
    }
}

pub struct Controller<S> {
    driver: MonoDriver,
    source: S,
    imgproc: Option<Imgproc>, // initialize on first frame, for correct pitch

    loaded_frame: Option<ImageBuffer<1>>,

    display_dirty_range: (i32, i32), // dirty row range
    display_full_refreshed: bool,
    displaying_row_map: Vec<bool>,
}

const EMPTY_DISPLAY_DIRTY_RANGE: (i32, i32) = (i32::MAX, i32::MIN);
const DRIVER_POLL_READY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);
const SOURCE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
const FULL_REFRESH_IDLE_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

impl<S> Controller<S>
where
    S: Source,
{
    pub fn new(driver: MonoDriver, source: S) -> Controller<S> {
        let screen_size = driver.get_screen_size();
        Controller {
            driver,
            source,
            imgproc: None,
            loaded_frame: None,
            display_dirty_range: EMPTY_DISPLAY_DIRTY_RANGE,
            display_full_refreshed: false,
            displaying_row_map: vec![false; screen_size.height as usize],
        }
    }

    // get frame and load into driver,modify display_dirty_range
    // return true if it's modified (display required)
    fn load_frame(&mut self) -> anyhow::Result<bool> {
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
            })));
        }
        self.imgproc
            .as_ref()
            .unwrap()
            .process(&bgra_img, &mut new_frame);
        let t_imgproc = std::time::Instant::now();

        let modified_range = match &self.loaded_frame {
            None => Some((0, screen_size.height)),
            Some(loaded_frame) => compute_modified_row_range(&new_frame, loaded_frame),
        };
        if let Some(modified_range) = modified_range {
            let load_subimg = new_frame.subimg(
                (0, modified_range.0).into(),
                (screen_size.width, modified_range.1 - modified_range.0).into(),
            );
            self.driver
                .load_image_fullwidth(modified_range.0 as u32, &load_subimg)?;
            self.display_dirty_range = (
                i32::min(self.display_dirty_range.0, modified_range.0),
                i32::max(self.display_dirty_range.1, modified_range.1),
            );
            self.loaded_frame = Some(new_frame);
        }
        let t_loaded = std::time::Instant::now();

        debug!("New frame loaded, row {:?} loaded, row {:?} dirty accumulated. Cost: get frame: {:?}, imgproc: {:?}, load: {:?}",
               modified_range, self.display_dirty_range,
               t_got_frame - t_load_start,
               t_imgproc - t_got_frame,
               t_loaded - t_imgproc);
        Ok(self.display_dirty_range.1 > self.display_dirty_range.0)
    }

    // return if current display dirty range is actually not blocked by displaying rows
    fn can_display_nonoverlapping(&self) -> bool {
        (self.display_dirty_range.0..self.display_dirty_range.1)
            .all(|i| !self.displaying_row_map[i as usize])
    }

    fn poll_display_ready(&mut self, block: bool) -> anyhow::Result<bool> {
        while self.driver.read_busy_state()? {
            if !block {
                return Ok(false);
            }
            std::thread::sleep(DRIVER_POLL_READY_INTERVAL);
        }
        self.displaying_row_map.fill(false);
        return Ok(true);
    }

    fn do_display_nonblock(&mut self) -> anyhow::Result<()> {
        assert!(self.display_dirty_range.0 < self.display_dirty_range.1);
        self.driver.display_area(
            (0, self.display_dirty_range.0).into(),
            (
                self.driver.get_screen_size().width,
                self.display_dirty_range.1 - self.display_dirty_range.0,
            )
                .into(),
            DisplayMode::A2,
            false,
        )?;
        for i in self.display_dirty_range.0..self.display_dirty_range.1 {
            self.displaying_row_map[i as usize] = true;
        }
        self.display_dirty_range = EMPTY_DISPLAY_DIRTY_RANGE;
        self.display_full_refreshed = false;
        Ok(())
    }

    fn do_display_full_refresh_block(&mut self) -> anyhow::Result<()> {
        self.driver.display_area(
            (0, 0).into(),
            self.driver.get_screen_size(),
            DisplayMode::GC16,
            true,
        )?;
        self.displaying_row_map.fill(false);
        self.display_dirty_range = EMPTY_DISPLAY_DIRTY_RANGE;
        self.display_full_refreshed = true;
        Ok(())
    }

    pub fn run_forever(&mut self) -> anyhow::Result<()> {
        let mut t_last_update = std::time::Instant::now();
        loop {
            let need_display = self.load_frame()?;
            if !need_display {
                // frame not changed
                if t_last_update.elapsed() > FULL_REFRESH_IDLE_DELAY && !self.display_full_refreshed
                {
                    info!("Full refresh!");
                    self.poll_display_ready(/* block */ true)?;
                    self.do_display_full_refresh_block()?;
                } else {
                    std::thread::sleep(SOURCE_POLL_INTERVAL);
                }
                continue;
            }

            if !self.poll_display_ready(/* block */ false)? && !self.can_display_nonoverlapping() {
                // cannot display now. we would wait for ready and loop again to get the newest frame
                self.poll_display_ready(/* block */ true)?;
                continue;
            }

            self.do_display_nonblock()?;

            let t_update = std::time::Instant::now();
            debug!(
                "New frame displayed, interval: {:?}",
                t_update - t_last_update
            );
            t_last_update = t_update;
        }
    }
}
