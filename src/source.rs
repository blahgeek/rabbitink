mod xcbgrab;

use crate::image::*;

pub trait Source {
    fn get_frame(&mut self) -> anyhow::Result<ConstImageView<32>>;
}

pub use xcbgrab::XcbGrabSource;
