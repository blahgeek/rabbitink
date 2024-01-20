use log::{debug, info};
use std::cell::RefCell;
use std::hash::Hasher;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::imgproc::dithering;

use super::driver::it8915::{DisplayMode, MemMode, IT8915};
use super::image::*;
use super::imgproc::{rotate::rotate as rotate_image, MonoImgproc, MonoImgprocOptions, Rotation};
use super::run_mode::RunMode;
use super::source::Source;

type RowSet = std::collections::BTreeSet<i32>;

fn compute_row_hashes(m: &impl ConstImage) -> Vec<u64> {
    (0..m.height())
        .map(|y| {
            let ptr = m.ptr(y);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            hasher.write(unsafe {std::slice::from_raw_parts(ptr, m.pitch() as usize)});
            hasher.finish()
        })
        .collect()
}

fn compute_modified_rows(row_hashes_a: &[u64], row_hashes_b: &[u64]) -> RowSet {
    assert_eq!(row_hashes_a.len(), row_hashes_b.len());
    let modified_rows = (0..row_hashes_a.len() as i32).filter(|i| {
        row_hashes_a[*i as usize] != row_hashes_b[*i as usize]
    });
    RowSet::from_iter(modified_rows)
}

pub struct AppOptions {
    pub reload_flag: Arc<AtomicBool>,
    pub terminate_flag: Arc<AtomicBool>,
    pub get_run_mode_callback: Box<dyn Fn() -> RunMode>,

    pub driver_poll_ready_interval: std::time::Duration,
    pub source_poll_interval: std::time::Duration,

    pub rotation: Rotation,
}

pub struct App {
    driver: IT8915,
    source: Box<dyn Source>,
    options: AppOptions,
    current_run_mode: RunMode,
    mono_imgproc: Rc<RefCell<MonoImgproc>>,

    loaded_frame_row_hashes: Vec<u64>,  // hash of each row's pixel value of loaded frame
    dirty_rows: RowSet,
    displaying_rows: RowSet,
    full_refreshed: bool,
}

const FULL_REFRESH_IDLE_DELAY: std::time::Duration = std::time::Duration::from_secs(120);
const FULL_REFRESH_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3); // prevent duplicated full refresh request within this period, if nothing is changed
const TEXT_ROW_TYPICAL_HEIGHT: i32 = 40; // when considering "row ratio" below, "expand" each pixel row to this height,
                                         // so that the "row ratio" is more close to what we assume
const SLOW_REFRESH_ROW_RATIO_THRESHOLD: f32 = 0.5; // do a slow (e.g. DU instead of A2) refresh if more than this ratio of rows are changed

impl App {
    pub fn new(driver: IT8915, source: Box<dyn Source>, options: AppOptions) -> App {
        let current_run_mode = (options.get_run_mode_callback)();
        let mono_imgproc = MonoImgproc::new(MonoImgprocOptions {
            rotation: options.rotation,
            input_size: source.frame_size(),
            output_size: driver.get_screen_size(),
        });
        App {
            driver,
            source,
            options,
            current_run_mode,
            mono_imgproc: Rc::new(RefCell::new(mono_imgproc)),
            loaded_frame_row_hashes: Vec::new(),
            dirty_rows: RowSet::default(),
            displaying_rows: RowSet::default(),
            full_refreshed: false,
        }
    }

    fn load_frame_generic<ImgprocFn>(&mut self, imgproc_fn: ImgprocFn) -> anyhow::Result<()>
    where ImgprocFn: FnOnce(&dyn ConstImage) -> ImageBuffer,
    {
        let screen_size = self.driver.get_screen_size();
        let t_load_start = std::time::Instant::now();

        let bgra_img = self.source.get_frame()?;
        let t_got_frame = std::time::Instant::now();

        let new_frame = imgproc_fn(bgra_img.as_ref());
        let t_imgproc = std::time::Instant::now();

        let new_frame_row_hashes = compute_row_hashes(&new_frame);
        let mut modified_range = if self.loaded_frame_row_hashes.is_empty() {
            RowSet::from_iter(0..screen_size.height)
        } else {
            compute_modified_rows(&self.loaded_frame_row_hashes, &new_frame_row_hashes)
        };
        if !modified_range.is_empty() {
            let load_subimg = new_frame.subimg(
                (0, *modified_range.first().unwrap()).into(),
                (screen_size.width,
                 *modified_range.last().unwrap() - modified_range.first().unwrap() + 1).into(),
            );
            let load_offset = *modified_range.first().unwrap() as u32;
            match self.current_run_mode.mem_mode() {
                MemMode::Mem1bpp => {
                    if load_subimg.format() == ImageFormat::Mono1Bpp {
                        self.driver.load_image_fullwidth_1bpp(load_offset, &load_subimg)?;
                    } else {
                        let packed = convert::repack_mono(&load_subimg, ImageFormat::Mono1Bpp,
                                                          self.driver.get_mem_pitch(MemMode::Mem1bpp));
                        self.driver.load_image_fullwidth_1bpp(load_offset, &packed)?;
                    }
                },
                MemMode::Mem8bpp => {
                    if load_subimg.format() == ImageFormat::Mono8Bpp {
                        self.driver.load_image_fullwidth_8bpp(load_offset, &load_subimg)?;
                    } else {
                        let unpacked = convert::repack_mono(&load_subimg, ImageFormat::Mono8Bpp,
                                                            self.driver.get_mem_pitch(MemMode::Mem8bpp));
                        self.driver.load_image_fullwidth_8bpp(load_offset, &unpacked)?;
                    }
                },
            }
            self.dirty_rows.append(&mut modified_range);
            drop(modified_range);
            self.loaded_frame_row_hashes = new_frame_row_hashes;
        }
        let t_loaded = std::time::Instant::now();

        debug!("New mono frame loaded, {} rows dirty accumulated. Cost: get frame: {:?}, imgproc: {:?}, load: {:?}",
               self.dirty_rows.len(),
               t_got_frame - t_load_start,
               t_imgproc - t_got_frame,
               t_loaded - t_imgproc);
        Ok(())
    }

    // get frame and load into driver,modify display_dirty_range
    fn load_frame_mono(&mut self) -> anyhow::Result<()> {
        let screen_size = self.driver.get_screen_size();
        let mempitch_1bpp = self.driver.get_mem_pitch(MemMode::Mem1bpp);
        let dithering_method = self.current_run_mode.dithering_method().unwrap();
        let mono_imgproc = self.mono_imgproc.clone();
        self.load_frame_generic(
            |bgra_img| {
                let mut new_frame = ImageBuffer::new(ImageFormat::Mono1Bpp, screen_size.width, screen_size.height, Some(mempitch_1bpp));
                mono_imgproc.borrow_mut().process(bgra_img, &mut new_frame, dithering_method);
                new_frame
            }
        )
    }

    fn load_frame_gray(&mut self) -> anyhow::Result<()> {
        let screen_size = self.driver.get_screen_size();
        let rotation = self.options.rotation;
        self.load_frame_generic(
            |bgra_img| {
                let bgra_img = rotate_image(bgra_img, rotation, screen_size);
                dithering::floyd_steinberg(&bgra_img, dithering::GREY16_TARGET_COLOR_SPACE)
            }
        )
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
            std::thread::sleep(self.options.driver_poll_ready_interval);
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

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut t_last_update = std::time::Instant::now();
        let mut t_last_need_update: Option<std::time::Instant> = None;
        while !self.options.terminate_flag.swap(false, Ordering::Relaxed) {
            let reload_requested = self.options.reload_flag.swap(false, Ordering::Relaxed);
            if reload_requested {
                let new_run_mode = (self.options.get_run_mode_callback)();
                if new_run_mode != self.current_run_mode {
                    info!("Switching to new run mode: {:?}", new_run_mode);
                    self.poll_display_ready(/* block */ true)?;
                    self.driver.reset_display()?;
                    self.loaded_frame_row_hashes.clear();
                    self.current_run_mode = new_run_mode;
                }
            }

            let load_result = match self.current_run_mode {
                RunMode::Mono(_) | RunMode::MonoForce8bpp(_) => self.load_frame_mono(),
                RunMode::Gray => self.load_frame_gray(),
            };
            if load_result.is_err() {   // TODO: check the error type
                // frame not ready
                std::thread::sleep(self.options.source_poll_interval);
                continue;
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
                t_last_need_update = None;
                continue;
            }

            if !need_display {
                // frame not changed
                std::thread::sleep(self.options.source_poll_interval);
                continue;
            }

            if t_last_need_update.is_none() {
                t_last_need_update = Some(std::time::Instant::now());
            }

            if !self.poll_display_ready(/* block */ false)? && !self.can_display_nonoverlapping() {
                // cannot display now. we would wait for ready and loop again to get the newest frame
                self.poll_display_ready(/* block */ true)?;
                continue;
            }

            let displayed_mode = self.do_display_nonblock()?;

            let t_update = std::time::Instant::now();
            info!(
                "New frame displayed, process delay: {:?}, mode: {:?}",
                t_update - t_last_need_update.unwrap(),
                displayed_mode
            );
            t_last_update = t_update;
            t_last_need_update = None;
        }
        self.driver.reset_display()?;
        Ok(())
    }
}
