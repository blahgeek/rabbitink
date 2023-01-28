use std::path::PathBuf;

use clap::Parser;
use opencv as cv;
use opencv::prelude::*;
use log::info;
use rabbitink::driver;


#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(short, long, default_value_t = 2)]
    mode: u32,

    #[arg(long, default_value_t = 500)]
    width: i32,

    #[arg(long, default_value_t = 100)]
    height: i32,

    #[arg(short, long, default_value_t = 1.0)]
    scale: f64,
}


fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let dev_path = PathBuf::from(&args.device);
    let mut dev = driver::it8915::IT8915::open(&dev_path)?;
    dev.pmic_control(Some(2150), Some(true))?;
    dev.display_area(opencv::core::Rect2i::new(0, 0, 0, 0), 0, true)?;

    let mut img : cv::core::Mat1b = cv::core::Mat::new_rows_cols_with_default(
        args.height, args.width, cv::core::CV_8UC1, cv::core::Scalar::all(0xf0 as f64))?.try_into_typed()?;
    for n in 0.. {
        let start = std::time::Instant::now();
        let text = n.to_string();
        img.set(cv::core::Scalar::all(0xf0 as f64))?;
        cv::imgproc::put_text(&mut img, &text, cv::core::Point::new(0, args.height),
                              cv::imgproc::FONT_HERSHEY_SIMPLEX, args.scale,
                              cv::core::Scalar::all(0.0), 2, cv::imgproc::LINE_8, false)?;
        info!("clock: {} start", n);
        dev.load_image_area((0, 0), &img)?;
        dev.display_area(opencv::core::Rect2i::new(0, 0, args.width, args.height), args.mode, true)?;
        info!("clock: {} done, cost {:?}", n, start.elapsed());
    }

    Ok(())
}
