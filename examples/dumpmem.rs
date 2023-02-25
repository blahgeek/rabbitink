use std::io::Write;

use clap::Parser;
use log::info;
use rabbitink::driver::it8915::IT8915;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(short, long)]
    output: String,

    #[arg(long)]
    start: u32,

    #[arg(long)]
    len_kb: u32,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let mut dev = IT8915::open(&args.device)?;

    let mut output = std::fs::File::create(args.output)?;
    for addr in (args.start .. (args.start + args.len_kb * 1024)).step_by(1024) {
        let content = dev.read_mem::<1024>(addr)?;
        output.write_all(&content)?;
        info!("Read address {:08x}", addr);
    }

    Ok(())
}
