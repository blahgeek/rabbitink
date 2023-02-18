use std::path::PathBuf;

use clap::Parser;

use rabbitink::control::Controller;
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

    let mut controller = Controller::new(dev, source);
    controller.run_forever()
}
