use super::Source;
use crate::image::*;

struct FrameAdapter<'a> {
    header: ImageHeader<32>,
    frame: scrap::Frame<'a>,
    data_offset: usize,
}

impl<'a> HasImageHeader<32> for FrameAdapter<'a> {
    fn header(&self) -> ImageHeader<32> {
        self.header
    }
}

impl<'a> ConstImage<32> for FrameAdapter<'a> {
    fn data(&self) -> &[u8] {
        &self.frame[self.data_offset..]
    }
}

struct GenericSource {
    capture: scrap::Capturer,

    top_left: Point,
    size: Size,
}

impl<'a> GenericSource {
    pub fn new(_: &str, rect: Option<(Point, Size)>) -> anyhow::Result<GenericSource> {
        let display = scrap::Display::primary()?;
        let capture = scrap::Capturer::new(display)?;

        let top_left = rect.map(|(p, _)| p).unwrap_or((0, 0).into());
        let size = rect
            .map(|(_, s)| s)
            .unwrap_or((capture.width() as i32, capture.height() as i32).into());

        if top_left.x + size.width > capture.width() as i32
            || top_left.y + size.height > capture.height() as i32
        {
            anyhow::bail!(
                "Invalid rect: {:?}, screen size: {}x{}",
                rect,
                capture.width(),
                capture.height()
            );
        }

        Ok(GenericSource {
            capture,
            top_left,
            size,
        })
    }
}

impl Source for GenericSource {
    fn get_frame(&mut self) -> anyhow::Result<Box<dyn ConstImage<32> + '_>> {
        let (frame_w, frame_h) = (self.capture.width() as i32, self.capture.height() as i32);
        let frame = self.capture.frame()?;
        let pitch = frame.len() as i32 / frame_h;
        assert!(pitch >= frame_w * 4);

        let data_offset = (self.top_left.y * pitch + self.top_left.x * 4) as usize;
        Ok(Box::new(FrameAdapter {
            header: ImageHeader::<32>::new(
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
