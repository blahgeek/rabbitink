mod generic;

#[cfg(target_os = "linux")]
mod bindings;
#[cfg(target_os = "linux")]
mod ioctl;
#[cfg(target_os = "linux")]
mod linux;

trait DeviceIO {
    fn io_write(&mut self, cmd: &[u8], data: &[u8]) -> anyhow::Result<()>;
    fn io_read(&mut self, cmd: &[u8], data: &mut [u8]) -> anyhow::Result<()>;
}

pub struct Device {
    io: Box<dyn DeviceIO>,
}

fn as_bytes<T>(data: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((data as *const T) as *const u8, std::mem::size_of::<T>()) }
}

fn as_bytes_mut<T>(data: &mut T) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((data as *mut T) as *mut u8, std::mem::size_of::<T>()) }
}

impl Device {
    pub fn open(desc: &str) -> anyhow::Result<Device> {
        let usb_bus_addr_regex = regex::Regex::new(r"([0-9]+),([0-9]+)").unwrap();
        let device_io: Box<dyn DeviceIO> = if desc.is_empty() {
            Box::new(generic::GenericDeviceIO::new(None)?)
        } else if let Some(capture) = usb_bus_addr_regex.captures(desc) {
            let bus = capture.get(1).unwrap().as_str().parse::<u8>()?;
            let addr = capture.get(2).unwrap().as_str().parse::<u8>()?;
            Box::new(generic::GenericDeviceIO::new(Some((bus, addr)))?)
        } else {
            #[cfg(target_os = "linux")]
            {Box::new(linux::LinuxDeviceIO::open(std::path::Path::new(desc))?)}
            #[cfg(not(target_os = "linux"))]
            unimplemented!()
        };
        Ok(Device { io: device_io })
    }

    pub fn io_write_bytes<CMD>(&mut self, cmd: &CMD, data: &[u8]) -> anyhow::Result<()> {
        self.io.io_write(as_bytes(cmd), data)
    }

    pub fn io_write<CMD, DATA>(&mut self, cmd: &CMD, data: &DATA) -> anyhow::Result<()> {
        self.io.io_write(as_bytes(cmd), as_bytes(data))
    }

    pub fn io_read<CMD, DATA>(&mut self, cmd: &CMD, data: &mut DATA) -> anyhow::Result<()> {
        self.io.io_read(as_bytes(cmd), as_bytes_mut(data))
    }
}
