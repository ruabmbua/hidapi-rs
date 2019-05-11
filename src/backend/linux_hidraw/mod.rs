use crate::backend::{ApiBackend, ApiDevice, ApiDeviceInfo};
use crate::error::HidResult;
use std::io::{self, Read, Write};

pub struct HidrawBackend;

impl ApiBackend for HidrawBackend {
    type Device = Device;
    type DeviceInfo = DeviceInfo;
    type DeviceInfoIter = std::vec::IntoIter<DeviceInfo>;

    fn create() -> HidResult<Self> {
        unimplemented!();
    }
    fn open_device(&self, vid: u16, pid: u16) -> HidResult<Self::Device> {
        unimplemented!()
    }
    fn open_device_with_serial(&self, vid: u16, pid: u16, serial: &str) -> HidResult<Self::Device> {
        unimplemented!()
    }
    fn enumerate(&mut self) -> HidResult<Self::DeviceInfoIter> {
        unimplemented!()
    }
}

pub struct Device;
pub struct DeviceInfo;

impl Write for Device {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unimplemented!()
    }
    fn flush(&mut self) -> io::Result<()> {
        unimplemented!()
    }
}

impl Read for Device {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unimplemented!()
    }
}

impl ApiDeviceInfo for DeviceInfo {
    fn path(&self) -> Option<String> {
        unimplemented!()
    }
    fn vendor_id(&self) -> u16 {
        unimplemented!()
    }
    fn product_id(&self) -> u16 {
        unimplemented!()
    }
    fn serial_number(&self) -> Option<String> {
        unimplemented!()
    }
    fn release_number(&self) -> u16 {
        unimplemented!()
    }
    fn manufacturer_string(&self) -> Option<String> {
        unimplemented!()
    }
    fn product_string(&self) -> Option<String> {
        unimplemented!()
    }
    fn usage_page(&self) -> Option<u16> {
        unimplemented!()
    }
    fn usage(&self) -> u16 {
        unimplemented!()
    }
    fn interface_number(&self) -> i32 {
        unimplemented!()
    }
}

impl ApiDevice for Device {}
