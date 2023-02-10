use anyhow::bail;
use opencv as cv;
use cv::prelude::*;
use log::{warn, info};

use super::driver::it8915;
use super::source::Source;
use super::imgproc::dithering;

fn compute_modified_row_range(m_a: &cv::core::Mat, m_b: &cv::core::Mat) -> Option<(i32, i32)> {
    assert_eq!(m_a.size().unwrap(), m_b.size().unwrap());
    assert_eq!(m_a.typ(), cv::core::CV_8U);
    assert_eq!(m_b.typ(), cv::core::CV_8U);
    let row_modified_status : Vec<(usize, bool)> =
        (0..m_a.rows()).map(|y| {
            let ptr_a = m_a.ptr(y).unwrap();
            let ptr_b = m_b.ptr(y).unwrap();
            let cmp_res = unsafe { libc::memcmp(ptr_a as *const libc::c_void, ptr_b as *const libc::c_void, m_a.cols() as usize) };
            return cmp_res != 0
        })
        .enumerate()
        .collect();

    let start = row_modified_status.iter().find(|x| x.1).map(|x| x.0 as i32);
    if let Some(start) = start {
        let end = row_modified_status.iter().rfind(|x| x.1).map(|x| 1 + x.0 as i32).unwrap();
        Some((start, end))
    } else {
        None
    }
}

struct State<S> {
    loaded_frame: Option<cv::core::Mat1b>,  // black-white
    display_dirty_region: cv::core::Rect2i,

    display_full_refreshed: bool,

    dev: it8915::IT8915,
    source: S,
}

struct LoadFrameResult {
    t_load_start: std::time::Instant,
    t_got_frame: std::time::Instant,
    t_imgproc: std::time::Instant,
    t_loaded: std::time::Instant,
}

impl<S: Source> State<S> {
    fn load_frame(&mut self) -> anyhow::Result<LoadFrameResult> {
        let screen_size = self.dev.get_screen_size();

        let t_load_start = std::time::Instant::now();
        let mut t_got_frame = t_load_start;
        let mut t_imgproc = t_load_start;
        let mut new_frame: Option<cv::core::Mat1b> = None;

        self.source.get_frame(&mut |orig_full_frame: &cv::core::Mat| -> anyhow::Result<()> {
            t_got_frame = std::time::Instant::now();
            let rect = cv::core::Rect2i::new(
                0, 0, orig_full_frame.cols().min(screen_size.width), orig_full_frame.rows().min(screen_size.height));
            if !rect.empty() {
                let orig_frame = cv::core::Mat::roi(&orig_full_frame, rect)?;
                new_frame = Some(dithering::floyd_steinberg(&orig_frame.try_into_typed()?,
                                                            dithering::BW_TARGET_COLOR_SPACE));
                t_imgproc = std::time::Instant::now();
                Ok(())
            } else {
                bail!("Source did not return valid frame")
            }
        })?;

        let new_frame = new_frame.unwrap();
        let modified_range = match &self.loaded_frame {
            None => Some((0, new_frame.rows())),
            Some(loaded_frame) => compute_modified_row_range(new_frame.as_untyped(), loaded_frame.as_untyped()),
        };
        if let Some(modified_range) = modified_range {
            let modified_rect = cv::core::Rect2i::new(0, modified_range.0, new_frame.cols(), modified_range.1 - modified_range.0);
            let load_area = cv::core::Mat::roi(new_frame.as_untyped(), modified_rect)?;
            self.dev.load_image_area(modified_rect.tl(), &load_area.try_into_typed()?)?;

            self.display_dirty_region = cv::core::Rect2i::from_points(
                (0, i32::min(self.display_dirty_region.tl().y, modified_range.0)).into(),
                (new_frame.cols(), i32::max(self.display_dirty_region.br().y, modified_range.1)).into()
            );
            self.loaded_frame = Some(new_frame);
        }
        let t_loaded = std::time::Instant::now();

        Ok(LoadFrameResult { t_load_start, t_got_frame, t_imgproc, t_loaded })
    }

    fn display(&mut self) -> anyhow::Result<()> {
        self.dev.display_area(self.display_dirty_region, it8915::DisplayMode::A2, false)?;
        self.display_dirty_region = cv::core::Rect2i::new(0,0,0,0);
        self.display_full_refreshed = false;
        Ok(())
    }

    fn display_full_refresh(&mut self) -> anyhow::Result<()> {
        self.dev.display_area(cv::core::Rect2i::from_point_size((0, 0).into(), self.dev.get_screen_size()),
                              it8915::DisplayMode::GC16, true)?;
        self.display_dirty_region = cv::core::Rect2i::new(0,0,0,0);
        self.display_full_refreshed = true;
        Ok(())
    }
}


pub fn run_forever<T>(mut dev: it8915::IT8915, source: T) -> anyhow::Result<()>
where T: Source {
    // initialize
    dev.set_memory_mode(it8915::MemoryMode::Pack1bpp)?;
    dev.reset_display()?;

    let mut s = State {
        loaded_frame: None,
        display_dirty_region: cv::core::Rect2i::new(0,0,0,0),
        display_full_refreshed: true,
        dev, source,
    };

    let mut t_last_frame = std::time::Instant::now();
    loop {
        let load_frame_result = s.load_frame()?;
        if s.dev.read_busy_state()? {
            continue;
        }

        if s.display_dirty_region.empty() {
            // frame not changed
            if t_last_frame.elapsed() > std::time::Duration::from_secs(5) && !s.display_full_refreshed {
                info!("Full refresh!");
                s.display_full_refresh()?;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            continue;
        }

        s.display()?;
        let t_display_finish = std::time::Instant::now();

        info!("New frame displayed. Interval: {:?}. Cost: wait {:?}, get frame: {:?}, imgproc: {:?}, load: {:?}, display: {:?}",
              t_display_finish - t_last_frame,
              load_frame_result.t_load_start - t_last_frame,
              load_frame_result.t_got_frame - load_frame_result.t_load_start,
              load_frame_result.t_imgproc - load_frame_result.t_got_frame,
              load_frame_result.t_loaded - load_frame_result.t_imgproc,
              t_display_finish - load_frame_result.t_loaded);
        t_last_frame = t_display_finish;

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
