use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::Parser;

use rabbitink::control::{ControlOptions, Controller};
use rabbitink::driver::it8915::MonoDriver;
use rabbitink::source::XcbGrabSource;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(long, default_value_t = 0)]
    grab_offx: i32,

    #[arg(long, default_value_t = 0)]
    grab_offy: i32,

    #[arg(long)]
    vcom: u16,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let dev_path = PathBuf::from(&args.device);
    let mut dev = MonoDriver::open(&dev_path)?;
    dev.pmic_control(Some(args.vcom), Some(true))?;
    dev.reset_display()?;

    let source = XcbGrabSource::new(
        ":0.0",
        Some((
            (args.grab_offx, args.grab_offy).into(),
            dev.get_screen_size(),
        )),
    )?;

    let full_refresh_flag = Arc::new(AtomicBool::default());
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, full_refresh_flag.clone())?;

    let terminate_flag = Arc::new(AtomicBool::default());
    for s in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(s, terminate_flag.clone())?;
    }

    let mut controller = Controller::new(
        dev,
        source,
        ControlOptions {
            full_refresh_flag,
            terminate_flag,
        },
    );
    controller.run_loop()
}
