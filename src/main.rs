use std::path::PathBuf;

use opencv as cv;
use cv::prelude::*;
use log::info;
use clap::Parser;

use rabbitink::source::{PublishingSourceAdapter, X11GrabSource, Source};
use rabbitink::driver;
use rabbitink::imgproc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let dev_path = PathBuf::from(&args.device);
    let mut dev = driver::it8915::IT8915::open(&dev_path)?;
    dev.pmic_control(Some(2150), Some(true))?;
    dev.set_memory_mode(driver::it8915::MemoryMode::Pack1bpp)?;
    dev.reset_display()?;
    let screen_rect = cv::core::Rect2i::from_point_size((0, 0).into(), dev.get_screen_size());

    let mut source = PublishingSourceAdapter::new(X11GrabSource {
        width: 1448,
        height: 1072,
        framerate: 100,
        input: ":0.0".into(),
    })?;
    source.start()?;

    loop {
        let mut frame = cv::core::Mat::default();
        source.get_frame(&mut frame)?;
        if frame.empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        let bw_image = imgproc::dithering::floyd_steinberg(&frame.try_into_typed()?,
                                                           imgproc::dithering::BW_TARGET_COLOR_SPACE);
        dev.load_image_area((0, 0).into(), &bw_image)?;
        dev.display_area(screen_rect, driver::it8915::DisplayMode::A2, true)?;
        while dev.read_busy_state()? {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    // std::thread::sleep(std::time::Duration::from_secs(1));

    // cv::imgcodecs::imwrite("/tmp/rabbitink.png", &frame, &cv::core::Vector::new())?;

    // source.stop()?;

    // Ok(())
}
