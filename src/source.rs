mod xcbgrab;
mod generic;

use crate::image::*;

pub trait Source {
    // return BGRA
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>>;
}

pub use xcbgrab::XcbGrabSource;
