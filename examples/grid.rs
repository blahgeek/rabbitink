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

    #[arg(long, default_value_t = 50)]
    height: i32,

    #[arg(long, default_value_t = 64)]
    width: i32,
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

    let screen_size = dev.get_screen_size();
    let x_repeat = screen_size.width / args.width;
    let y_repeat = screen_size.height / args.height;

    let img : cv::core::Mat = cv::core::Mat::new_rows_cols_with_default(
        screen_size.height, screen_size.width, cv::core::CV_8UC1, 0xf0.into())?;
    for y in 0..y_repeat {
        for x in 0..x_repeat {
            let t_start = std::time::Instant::now();

            let rect = cv::core::Rect::new(x * args.width, y * args.height, args.width, args.height);
            let mut grid = cv::core::Mat::roi(&img, rect)?;
            grid.set(0xf0.into())?;

            let text = ((x + y) % 10).to_string();
            cv::imgproc::put_text(&mut grid, &text, (0, args.height - 1).into(),
                                  cv::imgproc::FONT_HERSHEY_SIMPLEX, 1.0,
                                  cv::core::Scalar::all(0.0), 2, cv::imgproc::LINE_8, false)?;
            cv::imgproc::rectangle(&mut grid, cv::core::Rect2i::new(0, 0, args.width, args.height),
                                   cv::core::Scalar::all(0.0), 2, cv::imgproc::LINE_8, 0)?;

            // load full line
            let line_rect = cv::core::Rect::new(0, y * args.height, screen_size.width, args.height);
            let line_grid = cv::core::Mat::roi(&img, line_rect)?;
            dev.load_image_area((0, args.height * y).into(), &line_grid.try_into_typed()?)?;
            dev.display_area(rect, driver::it8915::DisplayMode::A2, false)?;

            info!("cost {:?}", t_start.elapsed());
        }
    }

    Ok(())
}
