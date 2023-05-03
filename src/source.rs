mod generic;
#[cfg(target_os = "linux")]
mod xcbgrab;

#[cfg(target_os = "macos")]
mod quartz;

use crate::image::*;

pub trait Source {
    // return BGRA
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>>;
    fn frame_size(&self) -> Size;
}

pub fn create_source(
    display: Option<&str>,
    offset: Point,
    max_size: Option<Size>,
) -> anyhow::Result<Box<dyn Source>> {
    #[cfg(target_os = "linux")]
    if display.is_none() || display.unwrap().starts_with(":") {
        return Ok(Box::new(xcbgrab::XcbGrabSource::new(display, offset, max_size)?));
    }

    #[cfg(target_os = "macos")]
    if let Ok(display_id) = display.unwrap_or("0").parse::<usize>() {
        return Ok(Box::new(quartz::QuartzSource::new(display_id, offset, max_size)?));
    }

    let display_id = display.unwrap_or("0").parse::<usize>()?;
    let result = Box::new(generic::GenericSource::new(display_id, offset, max_size)?);
    // FIXME: In macOS, the source need some time to initialize
    std::thread::sleep(std::time::Duration::from_secs(1));
    return Ok(result);
}
