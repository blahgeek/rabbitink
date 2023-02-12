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

struct State<S> {
    loaded_frame: Option<ImageBuffer<1>>,
    display_dirty_range: (i32, i32), // dirty row range

    display_full_refreshed: bool,

    driver: MonoDriver,
    source: S,
    imgproc: Option<Imgproc>, // initialize on first frame, for correct pitch
}

const EMPTY_DISPLAY_DIRTY_RANGE: (i32, i32) = (i32::MAX, i32::MIN);

struct LoadFrameResult {
    t_load_start: std::time::Instant,
    t_got_frame: std::time::Instant,
    t_imgproc: std::time::Instant,
    t_loaded: std::time::Instant,
}

impl<S: Source> State<S> {
    fn load_frame(&mut self) -> anyhow::Result<LoadFrameResult> {
        let screen_size = self.driver.get_screen_size();
        let t_load_start = std::time::Instant::now();

        let rgba_img = self.source.get_frame()?;
        if rgba_img.size() != screen_size {
            bail!(
                "Source returned invalid sized frame: {:?}, screen size {:?}",
                rgba_img.size(),
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
                rgba_pitch: rgba_img.pitch(),
                bw_pitch: self.driver.get_mem_pitch(),
            })));
        }
        self.imgproc
            .as_ref()
            .unwrap()
            .process(&rgba_img, &mut new_frame);
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

        Ok(LoadFrameResult {
            t_load_start,
            t_got_frame,
            t_imgproc,
            t_loaded,
        })
    }

    fn display(&mut self) -> anyhow::Result<()> {
        assert!(self.display_dirty_range.0 < self.display_dirty_range.1);
        self.driver.display_area(
            (0, self.display_dirty_range.0).into(),
            (
                self.driver.get_screen_size().width,
                self.display_dirty_range.1 - self.display_dirty_range.0,
            )
                .into(),
            DisplayMode::A2,
            true,
        )?;
        self.display_dirty_range = EMPTY_DISPLAY_DIRTY_RANGE;
        self.display_full_refreshed = false;
        Ok(())
    }

    fn display_full_refresh(&mut self) -> anyhow::Result<()> {
        self.driver.display_area(
            (0, 0).into(),
            self.driver.get_screen_size(),
            DisplayMode::GC16,
            true,
        )?;
        self.display_dirty_range = EMPTY_DISPLAY_DIRTY_RANGE;
        self.display_full_refreshed = true;
        Ok(())
    }
}

pub fn run_forever<T>(driver: MonoDriver, source: T) -> anyhow::Result<()>
where
    T: Source,
{
    let mut s = State {
        loaded_frame: None,
        display_dirty_range: EMPTY_DISPLAY_DIRTY_RANGE,
        display_full_refreshed: true,
        imgproc: None,
        driver,
        source,
    };

    let mut t_last_frame = std::time::Instant::now();
    loop {
        let load_frame_result = s.load_frame()?;

        if s.display_dirty_range.0 >= s.display_dirty_range.1 {
            // frame not changed
            if t_last_frame.elapsed() > std::time::Duration::from_secs(5)
                && !s.display_full_refreshed
            {
                info!("Full refresh!");
                s.display_full_refresh()?;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            continue;
        }

        s.display()?;
        while s.driver.read_busy_state()? {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let t_display_finish = std::time::Instant::now();

        debug!("New frame displayed. Interval: {:?}. Cost: wait {:?}, get frame: {:?}, imgproc: {:?}, load: {:?}, display: {:?}",
              t_display_finish - t_last_frame,
              load_frame_result.t_load_start - t_last_frame,
              load_frame_result.t_got_frame - load_frame_result.t_load_start,
              load_frame_result.t_imgproc - load_frame_result.t_got_frame,
              load_frame_result.t_loaded - load_frame_result.t_imgproc,
              t_display_finish - load_frame_result.t_loaded);
        t_last_frame = t_display_finish;
    }
}
