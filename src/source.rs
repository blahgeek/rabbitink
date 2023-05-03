#[cfg(target_os = "linux")]
mod xcbgrab;

#[cfg(target_os = "macos")]
mod quartz;

mod scrap_generic;

use crate::image::*;

pub trait Source {
    // return BGRA
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>>;
    fn frame_size(&self) -> Size;
}


#[cfg(target_os = "linux")]
pub fn create_source(
    display: Option<&str>,
    offset: Point,
    max_size: Option<Size>,
) -> anyhow::Result<Box<dyn Source>> {
    Ok(Box::new(xcbgrab::XcbGrabSource::new(display, offset, max_size)?))
}

#[cfg(target_os = "macos")]
pub fn create_source(
    display: Option<&str>,
    offset: Point,
    max_size: Option<Size>,
) -> anyhow::Result<Box<dyn Source>> {
    let display_id = display.unwrap_or("0").parse::<usize>()?;
    Ok(Box::new(quartz::QuartzSource::new(display_id, offset, max_size)?))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn create_source(
    display: Option<&str>,
    offset: Point,
    max_size: Option<Size>,
) -> anyhow::Result<Box<dyn Source>> {
    let display_id = display.unwrap_or("0").parse::<usize>()?;
    Ok(Box::new(generic::GenericSource::new(display_id, offset, max_size)?))
}
