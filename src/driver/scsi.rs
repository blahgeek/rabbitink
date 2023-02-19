mod bindings;
mod ioctl;
mod linux;

trait DeviceIO {
    fn io_write(&mut self, cmd: &[u8], data: &[u8]) -> anyhow::Result<()>;
    fn io_read(&mut self, cmd: &[u8], data: &mut [u8]) -> anyhow::Result<()>;
}

pub struct Device {
    io: Box<dyn DeviceIO>,
}

fn as_bytes<T>(data: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts((data as *const T) as *const u8, std::mem::size_of::<T>())
    }
}

fn as_bytes_mut<T>(data: &mut T) -> &mut [u8] {
    unsafe {
        std::slice::from_raw_parts_mut((data as *mut T) as *mut u8, std::mem::size_of::<T>())
    }
}

impl Device {
    pub fn open(path: &std::path::Path) -> anyhow::Result<Device> {
        let device_io = if cfg!(target_os = "linux") {
            Box::new(linux::LinuxDeviceIO::open(path)?)
        } else {
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


