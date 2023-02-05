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

    #[arg(long)]
    no_repeat: bool,
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

    let x_repeat = if args.no_repeat {1} else {dev.get_screen_size().width / args.width};
    let y_repeat = if args.no_repeat {1} else {dev.get_screen_size().height / args.height};

    let mut img : cv::core::Mat1b = cv::core::Mat::new_rows_cols_with_default(
        args.height, args.width, cv::core::CV_8UC1, cv::core::Scalar::all(0xf0 as f64))?.try_into_typed()?;
    for n in 0.. {
        let start = std::time::Instant::now();
        let text = n.to_string();
        img.set(cv::core::Scalar::all(0xf0 as f64))?;
        cv::imgproc::put_text(&mut img, &text, cv::core::Point::new(0, args.height),
                              cv::imgproc::FONT_HERSHEY_SIMPLEX, args.scale,
                              cv::core::Scalar::all(0.0), 2, cv::imgproc::LINE_8, false)?;

        // let y = (n % y_repeat) * args.height;
        info!("clock: {}, start", n);
        for x in 0..x_repeat {
            for y in 0..y_repeat {
                dev.load_image_area((args.width * x, args.height * y).into(), &img)?;
            }
        }
        info!("clock: {}, loaded image, {:?}", n, start.elapsed());

        for x in 0..x_repeat {
            for y in 0..y_repeat {
                dev.display_area(opencv::core::Rect2i::new(args.width * x, args.height * y, args.width / 10, args.height),
                                 args.mode,
                                 false)?;
            }
        }
        info!("clock: {}, sended display request, {:?}", n, start.elapsed());

        while dev.read_busy_state()? {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        info!("clock: {} done, cost {:?}", n, start.elapsed());
    }

    Ok(())
}
