#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl<T> From<(T, T)> for Point
where
    T: Into<i32>,
{
    fn from(value: (T, T)) -> Self {
        Point {
            x: value.0.into(),
            y: value.1.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl<T> From<(T, T)> for Size
where
    T: Into<i32>,
{
    fn from(value: (T, T)) -> Self {
        Size {
            width: value.0.into(),
            height: value.1.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageFormat {
    Mono1Bpp,  // mono, 1 bit per pixel
    Mono8Bpp,  // mono, 8 bits per pixel
    BGRA,      // BGRA, 32 bits per pixel
    DoubleByte, // 16 bits per pixel, mostly testing
}

impl ImageFormat {
    pub fn bpp(&self) -> i32 {
        match self {
            Self::Mono1Bpp => 1,
            Self::Mono8Bpp => 8,
            Self::BGRA => 32,
            Self::DoubleByte => 16,
        }
    }
}

fn minimum_pitch(bpp: i32, width: i32) -> i32 {
    (width * bpp + 7) / 8
}


#[derive(Clone, Copy, Debug)]
pub struct ImageHeader {
    data_len: usize,
    format: ImageFormat,
    width: i32,
    pitch: i32,
    height: i32,
}

impl ImageHeader {
    pub fn new(format: ImageFormat, data_len: usize, width: i32, height: i32, pitch: Option<i32>) -> Self {
        let minimum_pitch = minimum_pitch(format.bpp(), width);
        let pitch = pitch.unwrap_or(minimum_pitch);
        assert!(
            pitch >= minimum_pitch,
            "invalid pitch {} for width {} with format {:?}",
            pitch,
            width,
            format
        );
        assert!(
            width > 0 && height > 0,
            "invalid width {} and height {}",
            width,
            height
        );
        assert!(
            data_len >= (height * pitch) as usize,
            "invalid data len {} for height {} and pitch {}",
            data_len,
            height,
            pitch
        );
        ImageHeader {
            data_len,
            format,
            width,
            pitch,
            height,
        }
    }

    fn subimg(&self, pt: Point, size: Size) -> (Self, usize) {
        assert!((pt.x * self.format.bpp()) % 8 == 0);
        assert!(pt.x >= 0 && pt.x < self.width && pt.x + size.width <= self.width);
        assert!(pt.y >= 0 && pt.y < self.height && pt.y + size.height <= self.height);
        let offset = (pt.y * self.pitch + pt.x * self.format.bpp() / 8) as usize;
        (
            Self {
                data_len: self.data_len - offset,
                format: self.format,
                width: size.width,
                pitch: self.pitch,
                height: size.height,
            },
            offset,
        )
    }
}

pub trait HasImageHeader {
    fn header(&self) -> ImageHeader;
}

pub trait ConstImage: HasImageHeader {
    fn data(&self) -> &[u8];

    fn bpp(&self) -> i32 {
        self.header().format.bpp()
    }
    fn format(&self) -> ImageFormat {
        self.header().format
    }
    fn width(&self) -> i32 {
        self.header().width
    }
    fn height(&self) -> i32 {
        self.header().height
    }
    fn pitch(&self) -> i32 {
        self.header().pitch
    }
    fn is_continuous(&self) -> bool {
        self.pitch() == minimum_pitch(self.bpp(), self.width())
    }
    fn size(&self) -> Size {
        (self.width(), self.height()).into()
    }
    fn ptr(&self, row: i32) -> *const u8 {
        self.data()[((row * self.pitch()) as usize)..].as_ptr()
    }
    fn subimg(&self, pt: Point, size: Size) -> ConstImageView {
        let (sub_hdr, offset) = self.header().subimg(pt, size);
        ConstImageView {
            header: sub_hdr,
            data: &self.data()[offset..],
        }
    }
    fn view(&self) -> ConstImageView {
        ConstImageView {
            header: self.header(),
            data: self.data(),
        }
    }

}

pub trait Image: ConstImage {
    fn mut_data(&mut self) -> &mut [u8];

    fn mut_ptr(&mut self, row: i32) -> *mut u8 {
        let offset = (row * self.pitch()) as usize;
        self.mut_data()[offset..].as_mut_ptr()
    }
    fn mut_subimg(&mut self, pt: Point, size: Size) -> ImageView {
        let (sub_hdr, offset) = self.header().subimg(pt, size);
        ImageView {
            header: sub_hdr,
            data: &mut self.mut_data()[offset..],
        }
    }
    fn copy_from<T: ConstImage + ?Sized>(&mut self, src: &T) {
        assert_eq!(self.size(), src.size());
        let copy_len = minimum_pitch(self.bpp(), self.width()) as usize;
        for y in 0..self.height() {
            let dst_slice = unsafe { std::slice::from_raw_parts_mut(self.mut_ptr(y), copy_len) };
            let src_slice = unsafe { std::slice::from_raw_parts(src.ptr(y), copy_len) };
            dst_slice.copy_from_slice(src_slice);
        }
    }

    fn fill(&mut self, val: u8) {
        for y in 0..self.height() {
            let slice = unsafe { std::slice::from_raw_parts_mut(self.mut_ptr(y), self.pitch() as usize) };
            slice.fill(val);
        }
    }

    fn mut_view(&mut self) -> ImageView {
        ImageView {
            header: self.header(),
            data: self.mut_data(),
        }
    }
}

pub struct ConstImageView<'a> {
    header: ImageHeader,
    data: &'a [u8],
}

impl<'a> ConstImageView<'a> {
    pub fn new(format: ImageFormat, data: &'a [u8], width: i32, height: i32, pitch: Option<i32>) -> Self {
        let header = ImageHeader::new(format, data.len(), width, height, pitch);
        ConstImageView { header, data }
    }
}

impl<'a> HasImageHeader for ConstImageView<'a> {
    fn header(&self) -> ImageHeader {
        self.header
    }
}

impl<'a> ConstImage for ConstImageView<'a> {
    fn data(&self) -> &[u8] {
        self.data
    }
}

pub struct ImageView<'a> {
    header: ImageHeader,
    data: &'a mut [u8],
}

impl<'a> ImageView<'a> {
    pub fn new(format: ImageFormat, data: &'a mut [u8], width: i32, height: i32, pitch: Option<i32>) -> Self {
        let header = ImageHeader::new(format, data.len(), width, height, pitch);
        ImageView { header, data }
    }
}

impl<'a> HasImageHeader for ImageView<'a> {
    fn header(&self) -> ImageHeader {
        self.header
    }
}

impl<'a> ConstImage for ImageView<'a> {
    fn data(&self) -> &[u8] {
        self.data
    }
}

impl<'a> Image for ImageView<'a> {
    fn mut_data(&mut self) -> &mut [u8] {
        self.data
    }
}

pub struct ImageBuffer {
    data: Vec<u8>,
    header: ImageHeader,
}

impl ImageBuffer {
    pub fn new(format: ImageFormat, width: i32, height: i32, pitch: Option<i32>) -> Self {
        let minimum_pitch = minimum_pitch(format.bpp(), width);
        let pitch = pitch.unwrap_or(minimum_pitch);
        let data = vec![0; (pitch * height) as usize];
        let header = ImageHeader::new(format, data.len(), width, height, Some(pitch));
        Self { data, header }
    }
}

impl HasImageHeader for ImageBuffer {
    fn header(&self) -> ImageHeader {
        self.header
    }
}

impl ConstImage for ImageBuffer {
    fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

impl Image for ImageBuffer {
    fn mut_data(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1bpp() {
        let mut buf = ImageBuffer::new(ImageFormat::Mono1Bpp, 100, 100, None);
        let view = buf.mut_view();
        assert_eq!(view.width(), 100);
        assert_eq!(view.height(), 100);
        assert_eq!(view.pitch(), 13);
        let ptr = view.ptr(0);

        let sub0 = view.subimg((8, 2).into(), (64, 10).into());
        assert_eq!(sub0.width(), 64);
        assert_eq!(sub0.height(), 10);
        assert_eq!(sub0.pitch(), 13);
        assert_eq!(unsafe { ptr.add(13 * 2 + 1) }, sub0.ptr(0));
    }
}

pub mod convert;

#[cfg(feature = "opencv")]
pub mod cv_adapter;
