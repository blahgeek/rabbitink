use std::path::PathBuf;

use clap::Parser;

use rabbitink::source::XcbGrabSource;
use rabbitink::driver::it8915::MonoDriver;
use rabbitink::control;

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
    let mut dev = MonoDriver::open(&dev_path)?;
    dev.pmic_control(Some(2150), Some(true))?;

    let source = XcbGrabSource::new(":0.0", Some(((0, 0).into(), dev.get_screen_size())))?;

    control::run_forever(dev, source)
}
