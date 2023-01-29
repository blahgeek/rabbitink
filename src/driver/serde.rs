use std::fmt::Debug;

#[repr(packed)]
#[derive(Clone, Copy, Default)]
pub struct BigEndianU32 ([u8; 4]);

impl BigEndianU32 {
    pub fn val(&self) -> u32 {
        ((self.0[0] as u32) << 24) | ((self.0[1] as u32) << 16) | ((self.0[2] as u32) << 8) | (self.0[3] as u32)
    }
}

impl From<u32> for BigEndianU32 {
    fn from(value: u32) -> Self {
        Self([(value >> 24) as u8, ((value >> 16) & 0xff) as u8, ((value >> 8) & 0xff) as u8, (value & 0xff) as u8])
    }
}

impl Debug for BigEndianU32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [0x{:08x}, BE]", self.val(), self.val())
    }
}

#[repr(packed)]
#[derive(Clone, Copy, Default)]
pub struct BigEndianU16 ([u8; 2]);

impl BigEndianU16 {
    pub fn val(&self) -> u16 {
        ((self.0[0] as u16) << 8) | (self.0[1] as u16)
    }
}

impl From<u16> for BigEndianU16 {
    fn from(value: u16) -> Self {
        Self([((value >> 8) & 0xff) as u8, (value & 0xff) as u8])
    }
}

impl Debug for BigEndianU16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [0x{:04x}, BE]", self.val(), self.val())
    }
}
