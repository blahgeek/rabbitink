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

    #[arg(short, long, default_value = "gc16")]
    mode: driver::it8915::DisplayMode,

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
    dev.reset_display()?;

    let mut img : cv::core::Mat1b = cv::core::Mat::new_rows_cols_with_default(
        args.height, args.width, cv::core::CV_8UC1, cv::core::Scalar::all(0xf0 as f64))?.try_into_typed()?;
    for n in 0.. {
        let start = std::time::Instant::now();
        let text = n.to_string();
        img.set(cv::core::Scalar::all(0xf0 as f64))?;
        cv::imgproc::put_text(&mut img, &text, cv::core::Point::new(0, args.height),
                              cv::imgproc::FONT_HERSHEY_SIMPLEX, args.scale,
                              cv::core::Scalar::all(0.0), 2, cv::imgproc::LINE_8, false)?;

        let y = (n % (dev.screen_size().1 / args.height)) * args.height;
        let x_repeat = dev.screen_size().0 / args.width;
        info!("clock: {} start", n);
        for i in 0..x_repeat {
            dev.load_image_area(((args.width * i) as u32, y as u32), &img)?;
        }
        // dev.load_image_area((args.width as u32, y as u32), &img)?;
        for i in 0..x_repeat {
            dev.display_area(opencv::core::Rect2i::new(args.width * i, y, args.width, args.height), args.mode, false)?;
        }
        // dev.display_area(opencv::core::Rect2i::new(args.width, y, args.width, args.height), args.mode, true)?;
        while dev.read_busy_state()? {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        info!("clock: {} done, cost {:?}", n, start.elapsed());
    }

    Ok(())
}
