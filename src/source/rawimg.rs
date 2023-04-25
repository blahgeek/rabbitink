use std::{
    io::Read,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use log::error;

use super::Source;
use crate::image::*;

pub struct RawimgSource {
    current_frame: ImageBuffer<32>,
    next_frame: Arc<Mutex<Option<ImageBuffer<32>>>>,

    stop_flag: Arc<AtomicBool>,
    recv_thread: Option<std::thread::JoinHandle<()>>,
}

fn read_loop(
    size: Size,
    next_frame: &Mutex<Option<ImageBuffer<32>>>,
    stop_flag: &AtomicBool,
) -> anyhow::Result<()> {
    while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
        let mut new_frame = ImageBuffer::<32>::new(size.width, size.height, None);
        assert!(new_frame.is_continuous());

        std::io::stdin().read_exact(new_frame.mut_data())?;
        *next_frame.lock().unwrap() = Some(new_frame);
    }
    Ok(())
}

impl Drop for RawimgSource {
    fn drop(&mut self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        self.recv_thread.take().unwrap().join().unwrap();
    }
}

impl RawimgSource {
    pub fn new(size: Size) -> anyhow::Result<Self> {
        let next_frame = Arc::new(Mutex::new(None));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let recv_thread = std::thread::spawn({
            let next_frame = next_frame.clone();
            let stop_flag = stop_flag.clone();
            move || {
                if let Err(err) = read_loop(size, &next_frame, &stop_flag) {
                    error!("Rawimg read loop exited: {}", err);
                }
            }
        });

        Ok(RawimgSource {
            current_frame: ImageBuffer::<32>::new(size.width, size.height, None),
            next_frame,
            stop_flag,
            recv_thread: Some(recv_thread),
        })
    }
}

impl Source for RawimgSource {
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>> {
        if let Some(next_frame) = self.next_frame.lock().unwrap().take() {
            self.current_frame = next_frame;
        }
        Ok(Box::new(self.current_frame.view()))
    }
    fn frame_size(&self) -> Size {
        self.current_frame.size()
    }
}

