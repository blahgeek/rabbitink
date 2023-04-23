use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::Parser;

use rabbitink::app::{App, AppOptions};
use rabbitink::driver::it8915::IT8915;
use rabbitink::imgproc::Rotation;
use rabbitink::source;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, short, default_value = "")]
    device: String,

    #[arg(long, short)]
    source: Option<String>,

    #[arg(long, default_value_t = 0)]
    source_offx: i32,

    #[arg(long, default_value_t = 0)]
    source_offy: i32,

    #[arg(long)]
    vcom: f32,

    #[arg(long, short, default_value = "no-rotation")]
    rotation: Rotation,

    #[arg(long, default_value_t = 1)]
    driver_poll_ready_interval: u64,

    #[arg(long, default_value_t = 10)]
    source_poll_interval: u64,

    #[arg(long, default_value = "/tmp/rabbitink_run_mode.config")]
    run_mode_config: std::path::PathBuf,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let mut dev = IT8915::open(&args.device)?;
    dev.pmic_control(Some(args.vcom), Some(true))?;
    dev.reset_display()?;

    let source = source::create_source(
        args.source.as_deref(),
        Some((
            (args.source_offx, args.source_offy).into(),
            args.rotation.rotated_size(dev.get_screen_size()),
        )),
    )?;

    let reload_flag = Arc::new(AtomicBool::default());
    for s in [signal_hook::consts::SIGUSR1, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(s, reload_flag.clone())?;
    }

    let terminate_flag = Arc::new(AtomicBool::default());
    for s in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(s, terminate_flag.clone())?;
    }

    let mut app = App::new(
        dev,
        source,
        AppOptions {
            reload_flag,
            terminate_flag,
            run_mode_config_path: args.run_mode_config,
            driver_poll_ready_interval: std::time::Duration::from_millis(
                args.driver_poll_ready_interval,
            ),
            source_poll_interval: std::time::Duration::from_millis(
                args.source_poll_interval,
            ),
            rotation: args.rotation,
        },
    );
    app.run()
}
