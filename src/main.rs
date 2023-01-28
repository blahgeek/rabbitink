use std::path::PathBuf;

use rabbitink::driver;
use rabbitink::imgproc::dithering;


fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let img_orig = opencv::imgcodecs::imread(&std::env::args().nth(2).unwrap(),
                                             opencv::imgcodecs::IMREAD_GRAYSCALE)?;
    let img = dithering::floyd_steinberg(img_orig.try_into_typed().unwrap(),
                                         dithering::BW_TARGET_COLOR_SPACE);

    let dev_path = PathBuf::from(std::env::args().nth(1).unwrap());
    let mut dev = driver::it8915::IT8915::open(&dev_path)?;
    dev.pmic_control(Some(2150), Some(true))?;
    dev.display_area(opencv::core::Rect2i::new(0, 0, 0, 0), driver::it8915::DisplayMode::INIT, true)?;

    dev.load_image_area((0, 0), &img)?;
    dev.display_area(opencv::core::Rect2i::new(0, 0, 1448, 1072), driver::it8915::DisplayMode::A2, true)?;

    Ok(())
}
