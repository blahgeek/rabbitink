use super::DeviceIO;
use log::info;
use anyhow::Context;

pub struct GenericDeviceIO {
    dev: rusb::DeviceHandle<rusb::GlobalContext>,

    next_tag: u32,
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy)]
struct CBW {
    // Command Block
    signature: [u8; 4],
    tag: u32,
    data_len: u32, // little endian!
    direction: u8,
    lun: u8,
    cdb_len: u8,
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy)]
struct CSW {
    // Command Status
    signature: [u8; 4],
    tag: u32,
    residue: u32,
    status: u8,
}

const CBW_DIRECTION_BULK_IN: u8 = 0x80;
const CBW_DIRECTION_BULK_OUT: u8 = 0x00;

const ENDPOINT_OUT: u8 = 0x02;
const ENDPOINT_IN: u8 = 0x81;

const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

const VENDOR_ID: u16 = 0x048d;
const PRODUCT_ID: u16 = 0x8951;

impl GenericDeviceIO {
    fn new_with_rusb_device_handle(mut dev: rusb::DeviceHandle<rusb::GlobalContext>) -> anyhow::Result<Self> {
        dev.reset()?;
        dev.set_auto_detach_kernel_driver(true)?;
        dev.claim_interface(0)?;
        Ok(GenericDeviceIO { dev, next_tag: 0 })
    }

    pub fn new(bus_and_addr: Option<(u8, u8)>) -> anyhow::Result<GenericDeviceIO> {
        let dev = rusb::devices()?
            .iter()
            .filter(|dev| {
                let desc = dev.device_descriptor().unwrap();
                desc.vendor_id() == VENDOR_ID
                    && desc.product_id() == PRODUCT_ID
                    && (bus_and_addr.is_none()
                        || (dev.bus_number(), dev.address()) == bus_and_addr.unwrap())
            })
            .next()
            .ok_or(anyhow::format_err!("Cannot find target device {:?}", bus_and_addr))?;
        info!("Opening USB device {:?}", dev);
        Self::new_with_rusb_device_handle(dev.open()?)
    }

    fn pack_cbw_and_cdb(&mut self, cmd: &[u8], data_len: usize, direction: u8) -> Vec<u8> {
        let cbw = CBW {
            signature: [0x55, 0x53, 0x42, 0x43],
            tag: self.next_tag,
            data_len: u32::try_from(data_len).expect("data len too long"),
            direction,
            lun: 0,
            cdb_len: u8::try_from(cmd.len()).expect("cmd len too long"),
        };
        let mut res: Vec<u8> = Vec::new();
        res.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                (&cbw as *const CBW) as *const u8,
                std::mem::size_of::<CBW>(),
            )
        });
        res.extend_from_slice(cmd);
        return res;
    }

    fn check_status(&mut self) -> anyhow::Result<()> {
        let mut csw = CSW::default();
        let csw_buf = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut csw as *mut CSW) as *mut u8,
                std::mem::size_of::<CSW>(),
            )
        };
        self.dev.read_bulk(ENDPOINT_IN, csw_buf, DEFAULT_TIMEOUT)?;
        if csw.tag != self.next_tag {
            anyhow::bail!("Invalid tag in CSW");
        }
        self.next_tag += 1;
        Ok(())
    }
}

impl DeviceIO for GenericDeviceIO {
    fn io_write(&mut self, cmd: &[u8], data: &[u8]) -> anyhow::Result<()> {
        let cbw_and_cdb = self.pack_cbw_and_cdb(cmd, data.len(), CBW_DIRECTION_BULK_OUT);
        self.dev
            .write_bulk(ENDPOINT_OUT, &cbw_and_cdb, DEFAULT_TIMEOUT)?;
        self.dev.write_bulk(ENDPOINT_OUT, data, DEFAULT_TIMEOUT)?;
        self.check_status()
    }

    fn io_read(&mut self, cmd: &[u8], data: &mut [u8]) -> anyhow::Result<()> {
        let cbw_and_cdb = self.pack_cbw_and_cdb(cmd, data.len(), CBW_DIRECTION_BULK_IN);
        self.dev
            .write_bulk(ENDPOINT_OUT, &cbw_and_cdb, DEFAULT_TIMEOUT).context("write_bulk")?;
        self.dev.read_bulk(ENDPOINT_IN, data, DEFAULT_TIMEOUT).context("read_bulk")?;
        self.check_status()
    }
}
