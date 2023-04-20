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

fn minimum_pitch<const BPP: i32>(width: i32) -> i32 {
    (width * BPP + 7) / 8
}


#[derive(Clone, Copy, Debug)]
pub struct ImageHeader<const BPP: i32> {
    data_len: usize,
    width: i32,
    pitch: i32,
    height: i32,
}

impl<const BPP: i32> ImageHeader<BPP> {
    pub fn new(data_len: usize, width: i32, height: i32, pitch: Option<i32>) -> Self {
        let minimum_pitch = minimum_pitch::<BPP>(width);
        let pitch = pitch.unwrap_or(minimum_pitch);
        assert!(
            pitch >= minimum_pitch,
            "invalid pitch {} for width {} with {} bits-per-pixel",
            pitch,
            width,
            BPP
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
            width,
            pitch,
            height,
        }
    }

    fn subimg(&self, pt: Point, size: Size) -> (Self, usize) {
        assert!((pt.x * BPP) % 8 == 0);
        assert!(pt.x >= 0 && pt.x < self.width && pt.x + size.width <= self.width);
        assert!(pt.y >= 0 && pt.y < self.height && pt.y + size.height <= self.height);
        let offset = (pt.y * self.pitch + pt.x * BPP / 8) as usize;
        (
            Self {
                data_len: self.data_len - offset,
                width: size.width,
                pitch: self.pitch,
                height: size.height,
            },
            offset,
        )
    }
}

pub trait HasImageHeader<const BPP: i32> {
    fn header(&self) -> ImageHeader<BPP>;
}

pub trait ConstImage<const BPP: i32>: HasImageHeader<BPP> {
    fn data(&self) -> &[u8];

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
        self.pitch() == minimum_pitch::<BPP>(self.width())
    }
    fn size(&self) -> Size {
        (self.width(), self.height()).into()
    }
    fn ptr(&self, row: i32) -> *const u8 {
        self.data()[((row * self.pitch()) as usize)..].as_ptr()
    }
    fn subimg(&self, pt: Point, size: Size) -> ConstImageView<BPP> {
        let (sub_hdr, offset) = self.header().subimg(pt, size);
        ConstImageView {
            header: sub_hdr,
            data: &self.data()[offset..],
        }
    }
}

pub trait Image<const BPP: i32>: ConstImage<BPP> {
    fn mut_data(&mut self) -> &mut [u8];

    fn mut_ptr(&mut self, row: i32) -> *mut u8 {
        let offset = (row * self.pitch()) as usize;
        self.mut_data()[offset..].as_mut_ptr()
    }
    fn mut_subimg(&mut self, pt: Point, size: Size) -> ImageView<BPP> {
        let (sub_hdr, offset) = self.header().subimg(pt, size);
        ImageView {
            header: sub_hdr,
            data: &mut self.mut_data()[offset..],
        }
    }
    fn copy_from<T: ConstImage<BPP> + ?Sized>(&mut self, src: &T) {
        assert_eq!(self.size(), src.size());
        let copy_len = minimum_pitch::<BPP>(self.width()) as usize;
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
}

pub struct ConstImageView<'a, const BPP: i32> {
    header: ImageHeader<BPP>,
    data: &'a [u8],
}

impl<'a, const BPP: i32> ConstImageView<'a, BPP> {
    pub fn new(data: &'a [u8], width: i32, height: i32, pitch: Option<i32>) -> Self {
        let header = ImageHeader::<BPP>::new(data.len(), width, height, pitch);
        ConstImageView { header, data }
    }
}

impl<'a, const BPP: i32> HasImageHeader<BPP> for ConstImageView<'a, BPP> {
    fn header(&self) -> ImageHeader<BPP> {
        self.header
    }
}

impl<'a, const BPP: i32> ConstImage<BPP> for ConstImageView<'a, BPP> {
    fn data(&self) -> &[u8] {
        self.data
    }
}

pub struct ImageView<'a, const BPP: i32> {
    header: ImageHeader<BPP>,
    data: &'a mut [u8],
}

impl<'a, const BPP: i32> ImageView<'a, BPP> {
    pub fn new(data: &'a mut [u8], width: i32, height: i32, pitch: Option<i32>) -> Self {
        let header = ImageHeader::<BPP>::new(data.len(), width, height, pitch);
        ImageView { header, data }
    }
}

impl<'a, const BPP: i32> HasImageHeader<BPP> for ImageView<'a, BPP> {
    fn header(&self) -> ImageHeader<BPP> {
        self.header
    }
}

impl<'a, const BPP: i32> ConstImage<BPP> for ImageView<'a, BPP> {
    fn data(&self) -> &[u8] {
        self.data
    }
}

impl<'a, const BPP: i32> Image<BPP> for ImageView<'a, BPP> {
    fn mut_data(&mut self) -> &mut [u8] {
        self.data
    }
}

pub struct ImageBuffer<const BPP: i32> {
    data: Vec<u8>,
    header: ImageHeader<BPP>,
}

impl<const BPP: i32> ImageBuffer<BPP> {
    pub fn new(width: i32, height: i32, pitch: Option<i32>) -> Self {
        let minimum_pitch = minimum_pitch::<BPP>(width);
        let pitch = pitch.unwrap_or(minimum_pitch);
        let data = vec![0; (pitch * height) as usize];
        let header = ImageHeader::<BPP>::new(data.len(), width, height, Some(pitch));
        Self { data, header }
    }

    pub fn view(&self) -> ConstImageView<BPP> {
        ConstImageView {
            header: self.header,
            data: self.data.as_slice(),
        }
    }

    pub fn mut_view(&mut self) -> ImageView<BPP> {
        ImageView {
            header: self.header,
            data: self.data.as_mut_slice(),
        }
    }
}

impl<const BPP: i32> HasImageHeader<BPP> for ImageBuffer<BPP> {
    fn header(&self) -> ImageHeader<BPP> {
        self.header
    }
}

impl<const BPP: i32> ConstImage<BPP> for ImageBuffer<BPP> {
    fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

impl<const BPP: i32> Image<BPP> for ImageBuffer<BPP> {
    fn mut_data(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1bpp() {
        let mut buf = ImageBuffer::<1>::new(100, 100, None);
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
