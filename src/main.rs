use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use clap::Parser;

use rabbitink::app::{App, AppOptions};
use rabbitink::control::*;
use rabbitink::driver::it8915::IT8915;
use rabbitink::imgproc::DitheringMethod;
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

    let mut dev = IT8915::open(&args.device)?;
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

    let run_mode = Arc::new(Mutex::new(RunMode::Mono(DitheringMethod::Bayers4)));
    {
        let run_mode = run_mode.clone();
        std::thread::spawn(move || {
            run_socket_control_server(
                std::path::Path::new("/tmp/rabbitink.sock"),
                run_mode.clone(),
            ).unwrap();
        });
    }

    let mut app = App::new(
        dev,
        source,
        AppOptions {
            full_refresh_flag,
            terminate_flag,
            run_mode,
        },
    );
    app.run()
}
