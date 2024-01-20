use log::info;

use super::Source;
use crate::image::*;

struct FrameAdapter<'a> {
    header: ImageHeader,
    frame: scrap::Frame<'a>,
    data_offset: usize,
}

impl<'a> HasImageHeader for FrameAdapter<'a> {
    fn header(&self) -> ImageHeader {
        self.header
    }
}

impl<'a> ConstImage for FrameAdapter<'a> {
    fn data(&self) -> &[u8] {
        &self.frame[self.data_offset..]
    }
}

pub struct ScrapGenericSource {
    capture: scrap::Capturer,

    top_left: Point,
    size: Size,
}

impl<'a> ScrapGenericSource {
    #[allow(dead_code)]
    pub fn new(
        display_id: usize,
        top_left: Point,
        max_size: Option<Size>,
    ) -> anyhow::Result<ScrapGenericSource> {
        let displays = scrap::Display::all()?;
        if display_id >= displays.len() {
            anyhow::bail!("Invalid display id: {}", display_id);
        }
        info!("Using generic source with display ID {}", display_id);
        let display = displays.into_iter().nth(display_id).unwrap();
        let capture = scrap::Capturer::new(display)?;

        if top_left.x < 0
            || top_left.x >= capture.width() as i32
            || top_left.y < 0
            || top_left.y >= capture.height() as i32
        {
            anyhow::bail!(
                "Invalid top_left: {:?}, capture size={}x{}",
                top_left,
                capture.width(),
                capture.height()
            );
        }

        let size = Size {
            width: i32::min(
                max_size.map_or(0, |x| x.width),
                capture.width() as i32 - top_left.x,
            ),
            height: i32::min(
                max_size.map_or(0, |x| x.height),
                capture.height() as i32 - top_left.y,
            ),
        };

        Ok(ScrapGenericSource {
            capture,
            top_left,
            size,
        })
    }
}

impl Source for ScrapGenericSource {
    fn frame_size(&self) -> Size {
        self.size
    }

    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage + '_>> {
        let (frame_w, frame_h) = (self.capture.width() as i32, self.capture.height() as i32);
        let frame = self.capture.frame()?;
        let pitch = frame.len() as i32 / frame_h;
        assert!(pitch >= frame_w * 4);

        let data_offset = (self.top_left.y * pitch + self.top_left.x * 4) as usize;
        Ok(Box::new(FrameAdapter {
            header: ImageHeader::new(
                ImageFormat::BGRA,
                frame.len() as usize - data_offset,
                self.size.width,
                self.size.height,
                Some(pitch),
            ),
            frame,
            data_offset,
        }))
    }
}
