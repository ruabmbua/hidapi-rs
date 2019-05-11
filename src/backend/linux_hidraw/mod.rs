use crate::backend::{ApiBackend, ApiDevice, ApiDeviceInfo};
use crate::error::{HidError, HidResult, ResultExt};
use libudev::Context;
use std::io::{self, Read, Write};
mod udev_enumerator;

use udev_enumerator::{Enumerator, DeviceInfo};

pub struct HidrawBackend {
    udev_ctx: Context,
}

impl ApiBackend for HidrawBackend {
    type Device = Device;
    type DeviceInfo = DeviceInfo;
    type DeviceInfoIter = std::vec::IntoIter<DeviceInfo>;

    fn create() -> HidResult<Self> {
        let udev = Context::new().map_err(|e| HidError::UdevError { udev_e: e })?;
        Ok(Self { udev_ctx: udev })
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

impl ApiDevice for Device {}
