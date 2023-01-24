use std::path::PathBuf;

use opencv::prelude::*;
use log::trace;
use rabbitink::driver;


fn main() -> anyhow::Result<()> {
    env_logger::init();
    trace!("Hello, world!");

    let dev_path = PathBuf::from(std::env::args().nth(1).unwrap());
    let mut dev = driver::it8915::IT8915::open(&dev_path)?;

    // let img = opencv::imgcodecs::imread(&std::env::args().nth(2).unwrap(), opencv::imgcodecs::IMREAD_GRAYSCALE)?;
    // dev.load_image_area((0, 0), opencv::core::Mat::roi(&img, opencv::core::Rect2i::new(0, 0, 500, 500))?.try_into_typed()?)?;
    // dev.load_image_area((0, 0), opencv::core::Mat::zeros(500, 500, opencv::core::CV_8U)?.a().try_into_typed()?)?;
    // dev.display_area(opencv::core::Rect2i::new(0, 0, 500, 500), 2)?;

    dev.display_area(opencv::core::Rect2i::new(0, 0, 0, 0), 0)?;

    Ok(())
}
