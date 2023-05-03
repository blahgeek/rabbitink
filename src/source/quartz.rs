use std::{
    ops::{Deref, Index},
    sync::{Arc, Condvar, Mutex},
};

use log::info;
use scrap::quartz;

use super::Source;
use crate::image::*;

// NOTE: this is my own implementation of scrap::Capture using its internal implementation
// because I want to wait for next frame when not available, to reduce latency (avoid polling)
pub struct QuartzSource {
    capture: quartz::Capturer,
    top_left: Point,
    size: Size,

    current_frame: Option<quartz::Frame>,
    next: Arc<(Mutex<Option<quartz::Frame>>, Condvar)>,
}

impl QuartzSource {
    pub fn new(
        display_id: usize,
        top_left: Point,
        max_size: Option<Size>,
    ) -> anyhow::Result<QuartzSource> {
        let display = quartz::Display::online().unwrap().index(display_id).clone();
        info!("Using quartz source with display ID {}", display_id);

        if top_left.x < 0
            || top_left.x >= display.width() as i32
            || top_left.y < 0
            || top_left.y >= display.height() as i32
        {
            anyhow::bail!(
                "Invalid top_left: {:?}, capture size={}x{}",
                top_left,
                display.width(),
                display.height()
            );
        }

        let size = Size {
            width: i32::min(
                max_size.map_or(0, |x| x.width),
                display.width() as i32 - top_left.x,
            ),
            height: i32::min(
                max_size.map_or(0, |x| x.height),
                display.height() as i32 - top_left.y,
            ),
        };

        let next = Arc::new((Mutex::<Option<quartz::Frame>>::new(None), Condvar::new()));
        let capture = quartz::Capturer::new(
            display,
            display.width(),
            display.height(),
            quartz::PixelFormat::Argb8888,
            quartz::Config {
                cursor: true,
                ..Default::default()
            },
            {
                let next = next.clone();
                move |f| {
                    let mut locked = next.0.lock().unwrap();
                    *locked = Some(f);
                    next.1.notify_one();
                }
            },
        )
        .unwrap();

        Ok(QuartzSource {
            capture,
            top_left,
            size,
            current_frame: None,
            next,
        })
    }
}

const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

impl Source for QuartzSource {
    fn frame_size(&self) -> Size {
        self.size
    }

    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>> {
        // wait for a new frame
        {
            let locked = self.next.0.lock().unwrap();
            let (mut locked, wait_result) = self
                .next
                .1
                .wait_timeout_while(locked, WAIT_TIMEOUT, |f| f.is_none())
                .unwrap();
            if wait_result.timed_out() {
                anyhow::bail!("get_frame timeout");
            }
            self.current_frame = locked.take();
            assert!(self.current_frame.is_some());
        }

        let frame = self.current_frame.as_ref().unwrap();
        let data = &frame.deref()
            [(self.capture.width() * self.top_left.y as usize + self.top_left.x as usize) * 4..];
        Ok(Box::new(ConstImageView::new(
            data,
            self.size.width,
            self.size.height,
            None,
        )))
    }
}
