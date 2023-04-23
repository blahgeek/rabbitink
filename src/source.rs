#[cfg(target_os="linux")]
mod xcbgrab;
mod generic;
mod rawimg;

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

    if display.is_some() && display.unwrap() == "-" {
        if let Some((_, size)) = rect {
            return Ok(Box::new(rawimg::RawimgSource::new(size)?));
        } else {
            anyhow::bail!("Size must be specified when using rawimg source");
        }
    }

    let display_id = display.unwrap_or("0").parse::<usize>()?;
    return Ok(Box::new(generic::GenericSource::new(display_id, rect)?));
}
