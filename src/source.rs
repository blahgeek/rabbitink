#[cfg(target_os="linux")]
mod xcbgrab;
mod generic;

use crate::image::*;

pub trait Source {
    // return BGRA
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>>;
}

pub fn create_source(
    display: Option<&str>,
    rect: Option<(Point, Size)>,
) -> anyhow::Result<Box<dyn Source>> {

    #[cfg(target_os = "linux")]
    if display.is_none() || display.unwrap().starts_with(":") {
        return Ok(Box::new(xcbgrab::XcbGrabSource::new(display, rect)?));
    }

    let display_id = display.unwrap_or("0").parse::<usize>()?;
    let result = Box::new(generic::GenericSource::new(display_id, rect)?);
    // FIXME: In macOS, the source need some time to initialize
    std::thread::sleep(std::time::Duration::from_secs(1));
    return Ok(result);
}
