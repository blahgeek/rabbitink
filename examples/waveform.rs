use clap::Parser;
use rabbitink::driver::it8915::{MonoDriver, DisplayMode};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(short, long)]
    temperature: Option<u8>,

    #[arg(short, long, default_value = "init")]
    mode: DisplayMode,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let mut dev = MonoDriver::open(&args.device)?;

    if let Some(temp) = args.temperature {
        dev.set_force_temperature(temp)?;
    }
    println!("Temperature: {}", dev.read_temperature()?);

    dev.display_area((0, 0).into(), dev.get_screen_size(), args.mode, true)?;
    println!("{:?}", dev.read_current_waveform()?);

    Ok(())
}
