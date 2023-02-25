use std::io::Read;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::sync::Arc;

use log::warn;

use super::imgproc::DitheringMethod;
use super::driver::it8915::{DisplayMode, MemMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunMode {
    Mono(DitheringMethod),
    Gray,
}

impl RunMode {
    pub fn display_mode_fast(&self) -> DisplayMode {
        match self {
            &Self::Mono(_) => DisplayMode::A2,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn display_mode_slow(&self) -> DisplayMode {
        match self {
            &Self::Mono(_) => DisplayMode::DU,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn mem_mode(&self) -> MemMode {
        match self {
            &Self::Mono(_) => MemMode::Mem1bpp,
            &Self::Gray => MemMode::Mem8bpp,
        }
    }
}


const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

fn handle_client(mut stream: UnixStream, destination: Arc<Mutex<RunMode>>) -> anyhow::Result<()> {
    let mut content = String::new();
    stream.read_to_string(&mut content)?;

    let new_run_mode = match content.trim_end() {
        "mono_bayers4" => RunMode::Mono(DitheringMethod::Bayers4),
        "mono_bayers2" => RunMode::Mono(DitheringMethod::Bayers2),
        "mono_naive" => RunMode::Mono(DitheringMethod::NoDithering),
        "gray" => RunMode::Gray,
        _ => anyhow::bail!("Unsupported request: {}", content),
    };
    let mut dest = destination.lock().unwrap();
    *dest = new_run_mode;
    Ok(())
}

pub fn run_socket_control_server(sock_path: &std::path::Path,
                                 destination: Arc<Mutex<RunMode>>) -> anyhow::Result<()> {
    let listener = UnixListener::bind(sock_path)?;
    for stream in listener.incoming() {
        let stream = stream?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        if let Err(err) = handle_client(stream, destination.clone()) {
            warn!("Cannot handle client: {}", err);
        }
    }
    Ok(())
}
