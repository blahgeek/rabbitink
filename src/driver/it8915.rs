use std::fmt::Debug;

use anyhow::Context;
use log::{info, trace};

use super::scsi;
use super::serde::{BigEndianU16, BigEndianU32};
use super::waveform::Waveform;
use crate::image::*;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum DisplayMode {
    INIT = 0,
    DU,
    GC16,
    GL16,
    GLR16,
    GLD16,
    A2,
    DU4,
}

#[repr(packed)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
struct Sysinfo {
    standard_cmd_no: BigEndianU32,
    extend_cmd_no: BigEndianU32,
    signature: BigEndianU32,
    version: BigEndianU32,
    width: BigEndianU32,
    height: BigEndianU32,
    update_buf_base: BigEndianU32,
    image_buf_base: BigEndianU32,
    temperature_no: BigEndianU32,
    mode_no: BigEndianU32,
    frame_count: [BigEndianU32; 8],
    num_img_buf: BigEndianU32,
    reserved: [u32; 9],
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Default)]
struct MemIOCmd {
    hdr: u8,
    padding0: u8,
    addr: BigEndianU32,
    cmd: u8,
    len: BigEndianU16,
    padding1: [u8; 7],
}

#[repr(packed)]
#[derive(Default)]
#[allow(dead_code)]
struct PMICControlCmd {
    hdr: u8,
    padding1: [u8; 5],
    cmd: u8,
    vcom: BigEndianU16,
    vcom_set: u8,
    power_set: u8,
    power: u8,
    padding2: u32,
}

#[repr(packed)]
#[allow(dead_code)]
struct LoadImageAreaArgs {
    addr: BigEndianU32,
    x: BigEndianU32,
    y: BigEndianU32,
    w: BigEndianU32,
    h: BigEndianU32,
}

#[repr(packed)]
#[allow(dead_code)]
struct DisplayAreaArgs {
    addr: BigEndianU32,
    mode: BigEndianU32,
    x: BigEndianU32,
    y: BigEndianU32,
    w: BigEndianU32,
    h: BigEndianU32,
    wait_ready: BigEndianU32,
}

// always black-white only
pub struct IT8915 {
    device: scsi::Device,
    sysinfo: Sysinfo,
    mem_pitch: u32,
}

const EXPECT_INQUERY_VENDOR_PRODUCT: &'static str = "Generic Storage RamDisc 1.00";

impl IT8915 {
    pub fn get_screen_size(&self) -> Size {
        (
            self.sysinfo.width.val() as i32,
            self.sysinfo.height.val() as i32,
        )
            .into()
    }

    pub fn get_mem_pitch(&self) -> i32 {
        self.mem_pitch as i32
    }

    pub fn open(desc: &str) -> anyhow::Result<IT8915> {
        let mut device = scsi::Device::open(desc)?;

        // inquery, check vendor
        let mut inquery_cmd = [0_u8; 16];
        inquery_cmd[0] = 0x12;
        let mut inquery_result_buf = [0_u8; 40];
        device
            .io_read(&inquery_cmd, &mut inquery_result_buf)
            .context("failed to inquery")?;
        let inquery_result_vendor_product = String::from_utf8_lossy(&inquery_result_buf[8..36]);
        trace!("Inquery result: {}", inquery_result_vendor_product);
        if inquery_result_vendor_product != EXPECT_INQUERY_VENDOR_PRODUCT {
            anyhow::bail!(
                "unexpected vendor product string: {}",
                inquery_result_vendor_product
            );
        }

        let sysinfo_cmd: [u8; 16] = [
            0xfe, 0x00, 0x38, 0x39, 0x35, 0x31, // "8951"
            0x80, 0x00, 0x01, 0x00, 0x02, 0x00, // version: 0x00010002
            0x00, 0x00, 0x00, 0x00,
        ];
        let mut sysinfo = Sysinfo::default();
        device
            .io_read(&sysinfo_cmd, &mut sysinfo)
            .context("failed to read sysinfo")?;
        trace!("Sysinfo: {:?}", sysinfo);

        let mut res = IT8915 {
            device,
            sysinfo,
            mem_pitch: ((sysinfo.width.val() + 31) / 32) * 4, // 4byte align
        };

        // Enable/Disable 1bit drawing and image pitch mode
        // 0000 0000 0000 0110 0000 0000 0000 0000
        // |         |     ^^  |         |
        // 113B      113A      1139      1138
        let mut up1sr = res.read_mem::<4>(0x1800_1138)?;
        up1sr[2] |= 0x06;
        res.write_mem(0x1800_1138, &up1sr)?;

        // Set bitmap mode color definition (0 - set black(0x00), 1 - set white(0xf0))
        res.write_mem(0x1800_1250, &[0xf0, 0x00])?;

        // (not sure about why the "/4"... apparently the reg is in double-word)
        res.write_mem(
            0x1800_124c,
            &[
                ((res.mem_pitch / 4) & 0xff) as u8,
                (((res.mem_pitch / 4) >> 8) & 0xff) as u8,
            ],
        )?;

        Ok(res)
    }

    pub fn reset_display(&mut self) -> anyhow::Result<()> {
        let mut white_img = ImageBuffer::<1>::new(
            self.get_screen_size().width,
            self.get_screen_size().height,
            Some(self.mem_pitch as i32),
        );
        white_img.fill(0xff);

        // although INIT would flush the display regardless of the memory content,
        // if we don't initialize the memory content, the following display cannot work correctly,
        // apparently they would depend on the last state.
        self.load_image_fullwidth(0, &white_img)?;
        self.display_area(
            (0, 0).into(),
            self.get_screen_size(),
            DisplayMode::INIT,
            true,
        )
    }

    pub fn pmic_control(&mut self, vcom: Option<u16>, power: Option<bool>) -> anyhow::Result<()> {
        let mut cmd = PMICControlCmd {
            hdr: 0xfe,
            cmd: 0xa3,
            ..PMICControlCmd::default()
        };
        if let Some(vcom) = vcom {
            cmd.vcom = BigEndianU16::from(vcom);
            cmd.vcom_set = 1;
            info!("Setting VCOM value: {}", vcom);
        }
        if let Some(power) = power {
            cmd.power_set = 1;
            cmd.power = if power { 1 } else { 0 };
            info!("Setting power: {}", power);
        }
        self.device.io_write(&cmd, &())?;
        Ok(())
    }

    pub fn read_mem<const LEN: usize>(&mut self, addr: u32) -> anyhow::Result<[u8; LEN]> {
        let mut res: [u8; LEN] = [0_u8; LEN];
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0x81,
            len: BigEndianU16::from(u16::try_from(LEN).expect("read_mem buf too long")),
            ..MemIOCmd::default()
        };
        self.device.io_read(&cmd, &mut res)?;
        Ok(res)
    }

    pub fn read_busy_state(&mut self) -> anyhow::Result<bool> {
        let res = self.read_mem::<2>(0x18001224)?; // LUTAFSR + 0x18000000
        let busy = res.iter().any(|x| *x != 0);
        Ok(busy)
    }

    pub fn read_temperature(&mut self) -> anyhow::Result<u8> {
        let cmd: [u8; 16] = [0xfe, 0, 0, 0, 0, 0, 0xa4, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut res: [u8; 4] = [0; 4];
        self.device.io_read(&cmd, &mut res)?;
        Ok(res[0])
    }

    // NOTE: there doesn't seem to be a way to un-set the force temperature (aside from restart)
    pub fn set_force_temperature(&mut self, val: u8) -> anyhow::Result<()> {
        let cmd: [u8; 16] = [0xfe, 0, 0, 0, 0, 0, 0xa4, 0x01, val, 0, 0, 0, 0, 0, 0, 0];
        let mut res: [u8; 4] = [0; 4];
        self.device.io_read(&cmd, &mut res)?;
        Ok(())
    }

    fn write_mem(&mut self, addr: u32, data: &[u8]) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0x82,
            len: BigEndianU16::from(u16::try_from(data.len()).expect("write_mem buf too long")),
            ..MemIOCmd::default()
        };
        self.device.io_write_bytes(&cmd, data)?;
        Ok(())
    }

    fn write_mem_fast(&mut self, addr: u32, data: &[u8]) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0xa5,
            len: BigEndianU16::from(
                u16::try_from(data.len()).expect("write_mem_fast data too long"),
            ),
            ..MemIOCmd::default()
        };
        self.device.io_write_bytes(&cmd, data)?;
        Ok(())
    }

    // faster than load_image_area, but the image must cover full width
    pub fn load_image_fullwidth(
        &mut self,
        row_offset: u32,
        image: &impl ConstImage<1>,
    ) -> anyhow::Result<()> {
        trace!(
            "Loading image fullwidth to row {}, image size={:?}",
            row_offset,
            image.size()
        );
        assert_eq!(image.width(), self.get_screen_size().width);
        assert_eq!(image.pitch(), self.mem_pitch as i32);

        let rows_per_step = ((u16::MAX as u32) / self.mem_pitch) as i32;
        let mut y = 0;
        while y < image.height() {
            let subimg = image.subimg(
                (0, y).into(),
                (image.width(), i32::min(rows_per_step, image.height() - y)).into(),
            );
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    subimg.ptr(0),
                    (subimg.height() * subimg.pitch()) as usize,
                )
            };
            self.write_mem_fast(
                self.sysinfo.image_buf_base.val() + self.mem_pitch * (y as u32 + row_offset),
                &bytes,
            )?;

            y += rows_per_step;
        }

        Ok(())
    }

    // same as load_image_fullwidth, but accept 8bit image
    pub fn load_image_fullwidth_8bit(
        &mut self,
        row_offset: u32,
        image: &impl ConstImage<8>,
    ) -> anyhow::Result<()> {
        let packed = convert::pack(image, self.get_mem_pitch());
        self.load_image_fullwidth(row_offset, &packed)
    }

    pub fn display_area(
        &mut self,
        tl: Point,
        size: Size,
        mode: DisplayMode,
        wait_ready: bool,
    ) -> anyhow::Result<()> {
        trace!("Displaying region {:?} {:?}, mode = {:?}", tl, size, mode);
        assert!(tl.x % 32 == 0, "Pack1bpp mode requires 4 byte align");
        assert!(
            size.width % 32 == 0 || size.width == self.get_screen_size().width,
            "Pack1bpp mode requires 4 byte align"
        );

        let mode_val = match self.sysinfo.mode_no.val() {
            8 => mode as u32,
            6 => match mode {
                DisplayMode::INIT | DisplayMode::DU | DisplayMode::GC16 | DisplayMode::GL16 => {
                    mode as u32
                }
                DisplayMode::A2 | DisplayMode::DU4 => mode as u32 - 2,
                _ => anyhow::bail!("unsupported mode {:?} in this device", mode),
            },
            _ => anyhow::bail!(
                "unsupported device with mode_no {}",
                self.sysinfo.mode_no.val()
            ),
        };

        let cmd: [u8; 16] = [
            0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x94, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let args = DisplayAreaArgs {
            addr: self.sysinfo.image_buf_base,
            mode: BigEndianU32::from(mode_val),
            x: BigEndianU32::from(tl.x as u32),
            y: BigEndianU32::from(tl.y as u32),
            w: BigEndianU32::from(size.width as u32),
            h: BigEndianU32::from(size.height as u32),
            wait_ready: BigEndianU32::from(if wait_ready { 1 } else { 0 }),
        };

        self.device.io_write(&cmd, &args)?;
        Ok(())
    }

    // TODO: this does not seem stable. only valid for 6inch model
    const WAVEFORM_DATA_ADDR: u32 = 0x9c3e8;

    pub fn read_current_waveform(&mut self) -> anyhow::Result<Waveform> {
        const WAVEFORM_MAXLEN: usize = 256 * 64;
        let buf = self.read_mem::<WAVEFORM_MAXLEN>(IT8915::WAVEFORM_DATA_ADDR)?;
        let waveform = Waveform::new(&buf)?;

        // TODO: this does not seem stable
        // let endpointer_addr: u32 = 0x73464;
        // let endpointer_buf = self.read_mem::<4>(endpointer_addr)?;
        // trace!("endpointer: {:?}", BigEndianU32(endpointer_buf));
        // let expected_frames = (BigEndianU32(endpointer_buf).val() as i32 - data_addr as i32) / 64;
        // if waveform.frame_count() as i32 != expected_frames {
        //     anyhow::bail!(
        //         "unexpected waveform frame count: {} vs {}",
        //         waveform.frame_count(),
        //         expected_frames
        //     );
        // }
        Ok(waveform)
    }

    pub fn write_waveform(&mut self, waveform: &Waveform) -> anyhow::Result<()> {
        self.write_mem(IT8915::WAVEFORM_DATA_ADDR, waveform.data())
    }
}
