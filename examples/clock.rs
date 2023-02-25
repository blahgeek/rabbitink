use clap::Parser;
use log::info;
use opencv as cv;
use opencv::prelude::*;
use rabbitink::driver::it8915::{DisplayMode, IT8915, MemMode};
use rabbitink::image::cv_adapter;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(short, long, default_value = "gc16")]
    mode: DisplayMode,

    #[arg(long, default_value_t = 100)]
    height: i32,

    #[arg(short, long, default_value_t = 1.0)]
    scale: f64,

    #[arg(long, default_value_t = 0)]
    repeat: i32,

    #[arg(long, default_value_t = -1)]
    wait: i32,

    #[arg(long)]
    vcom: u16,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let mut dev = IT8915::open(&args.device)?;
    dev.pmic_control(Some(args.vcom), Some(true))?;
    dev.reset_display()?;

    let screen_size = dev.get_screen_size();
    let y_repeat = if args.repeat > 0 {
        args.repeat
    } else {
        dev.get_screen_size().height / args.height
    };

    let mut img: cv::core::Mat1b = cv::core::Mat::new_rows_cols_with_default(
        args.height,
        screen_size.width,
        cv::core::CV_8UC1,
        cv::core::Scalar::all(0xf0 as f64),
    )?
    .try_into_typed()?;
    for n in 0.. {
        let start = std::time::Instant::now();
        let text = n.to_string();
        img.set(cv::core::Scalar::all(0xf0 as f64))?;
        cv::imgproc::put_text(
            &mut img,
            &text,
            cv::core::Point::new(0, args.height),
            cv::imgproc::FONT_HERSHEY_SIMPLEX,
            args.scale,
            cv::core::Scalar::all(0.0),
            2,
            cv::imgproc::LINE_8,
            false,
        )?;

        info!("clock: {}, start", n);
        for y in 0..y_repeat {
            dev.load_image_fullwidth_1bpp_from_8bpp(
                (args.height * y) as u32,
                &cv_adapter::cvmat_image_view::<8>(img.as_untyped()),
            )?;
        }
        info!("clock: {}, loaded image, {:?}", n, start.elapsed());

        for y in 0..y_repeat {
            dev.display_area(
                (0, args.height * y).into(),
                (screen_size.width, args.height).into(),
                args.mode,
                MemMode::Mem1bpp,
                false,
            )?;
        }
        info!(
            "clock: {}, sended display request, {:?}",
            n,
            start.elapsed()
        );

        if args.wait < 0 {
            while dev.read_busy_state()? {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        } else if args.wait > 0 {
            std::thread::sleep(std::time::Duration::from_millis(args.wait as u64));
        }
        info!("clock: {} done, cost {:?}", n, start.elapsed());
    }

    Ok(())
}
