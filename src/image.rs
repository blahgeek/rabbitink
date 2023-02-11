pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl<T> From<(T, T)> for Point where T: Into<i32> {
    fn from(value: (T, T)) -> Self {
        Point { x: value.0.into(), y: value.1.into() }
    }
}

pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl<T> From<(T, T)> for Size where T: Into<i32> {
    fn from(value: (T, T)) -> Self {
        Size { width: value.0.into(), height: value.1.into() }
    }
}

fn default_pitch<const BITS_PER_PIXEL: i32>(width: i32) -> i32 {
    (width * BITS_PER_PIXEL + 7) / 8
}

pub struct Image<'a, const BITS_PER_PIXEL: i32> {
    data: &'a mut [u8],

    width: i32,
    pitch: i32,
    height: i32,
}

impl<'a, const BITS_PER_PIXEL: i32> Image<'a, BITS_PER_PIXEL> {
    pub fn new(
        data: &'a mut [u8],
        width: i32,
        height: i32,
        pitch: Option<i32>,
    ) -> Image<'a, BITS_PER_PIXEL> {
        let default_pitch = default_pitch::<BITS_PER_PIXEL>(width);
        let pitch = pitch.unwrap_or(default_pitch);
        assert!(
            pitch >= default_pitch,
            "invalid pitch {} for width {} with {} bits-per-pixel",
            pitch,
            width,
            BITS_PER_PIXEL
        );
        assert!(
            width > 0 && height > 0,
            "invalid width {} and height {}",
            width,
            height
        );
        assert!(
            data.len() >= (height * pitch) as usize,
            "invalid data len {} for height {} and pitch {}",
            data.len(),
            height,
            pitch
        );

        Image {
            data,
            width,
            pitch,
            height,
        }
    }

    pub fn width(&self) -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }
    pub fn pitch(&self) -> i32 { self.pitch }
    pub fn is_continuous(&self) -> bool { self.pitch == default_pitch::<BITS_PER_PIXEL>(self.width) }
    pub fn ptr(&self, row: i32) -> *const u8 { self.data[((row * self.pitch) as usize)..].as_ptr() }
    pub fn ptr_mut(&mut self, row: i32) -> *mut u8 { self.data[((row * self.pitch) as usize)..].as_mut_ptr() }

    pub fn subimg(&mut self, pt: Point, size: Size) -> Image<BITS_PER_PIXEL> {
        assert!((pt.x * BITS_PER_PIXEL) % 8 == 0);
        assert!(pt.x >= 0 && pt.x < self.width && pt.x + size.width < self.width);
        assert!(pt.y >= 0 && pt.y < self.height && pt.y + size.height < self.height);
        let offset = (pt.y * self.pitch + pt.x * BITS_PER_PIXEL / 8) as usize;
        Image { data: &mut self.data[offset..], width: size.width, pitch: self.pitch, height: size.height }
    }
}

pub struct ImageBuffer<const BITS_PER_PIXEL: i32> {
    data: Vec<u8>,
    width: i32,
    height: i32,
    pitch: i32,
}

impl<const BITS_PER_PIXEL: i32> ImageBuffer<BITS_PER_PIXEL> {
    pub fn new(width: i32, height: i32, pitch: Option<i32>) -> ImageBuffer<BITS_PER_PIXEL> {
        let default_pitch = default_pitch::<BITS_PER_PIXEL>(width);
        let pitch = pitch.unwrap_or(default_pitch);
        assert!(
            pitch >= default_pitch,
            "invalid pitch {} for width {} with {} bits-per-pixel",
            pitch,
            width,
            BITS_PER_PIXEL
        );
        assert!(
            width > 0 && height > 0,
            "invalid width {} and height {}",
            width,
            height
        );
        ImageBuffer { data: vec![0; (pitch * height) as usize], width, height, pitch }
    }

    pub fn view(&mut self) -> Image<BITS_PER_PIXEL> {
        Image { data: self.data.as_mut_slice(), width: self.width, pitch: self.pitch, height: self.height }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1bpp() {
        let mut buf = ImageBuffer::<1>::new(100, 100, None);
        let mut view = buf.view();
        assert_eq!(view.width, 100);
        assert_eq!(view.height, 100);
        assert_eq!(view.pitch, 13);
        let ptr = view.ptr(0);

        let sub0 = view.subimg((8, 2).into(), (64, 10).into());
        assert_eq!(sub0.width, 64);
        assert_eq!(sub0.height, 10);
        assert_eq!(sub0.pitch, 13);
        assert_eq!(unsafe {ptr.add(13 * 2 + 1)}, sub0.ptr(0));
    }
}
