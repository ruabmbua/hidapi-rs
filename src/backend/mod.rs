// **************************************************************************
// Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi-rs by Osspial
// **************************************************************************

#[cfg(feature = "linux-rust-hidraw")]
mod linux_hidraw;

#[cfg(feature = "linux-rust-hidraw")]
pub use self::linux_hidraw::HidrawBackend as Backend;

#[cfg(not(feature = "linux-rust-hidraw"))]
mod hidapi;

#[cfg(not(feature = "linux-rust-hidraw"))]
pub use self::hidapi::HidapiBackend as Backend;

#[cfg(not(feature = "linux-rust-hidraw"))]
pub use self::hidapi::libc as libc;

use crate::error::{HidError, HidResult};
use std::io::{Read, Write};

pub trait ApiBackend
where
    Self: Sized,
    Self::Device: ApiDevice + Read + Write,
    Self::DeviceInfo: ApiDeviceInfo,
    Self::DeviceInfoIter: Iterator<Item = Self::DeviceInfo>,
{
    type Device;
    type DeviceInfo;
    type DeviceInfoIter;

    fn create() -> HidResult<Self>;
    fn open_device(&self, vid: u16, pid: u16) -> HidResult<Self::Device>;
    fn open_device_with_serial(&self, vid: u16, pid: u16, serial: &str) -> HidResult<Self::Device>;
    fn enumerate(&mut self) -> HidResult<Self::DeviceInfoIter>;
}

pub trait ApiDevice: Write + Read {
    fn write_report_id(&mut self, report_id: u8, data: &[u8]) -> std::io::Result<usize> {
        let mut buf = Vec::with_capacity(data.len() + 1);
        buf.push(report_id);
        buf.extend_from_slice(data);

        self.write(buf.as_slice())
    }
}

pub trait ApiDeviceInfo {
    fn path(&self) -> Option<String>;
    fn vendor_id(&self) -> u16;
    fn product_id(&self) -> u16;
    fn serial_number(&self) -> Option<String>;
    fn release_number(&self) -> u16;
    fn manufacturer_string(&self) -> Option<String>;
    fn product_string(&self) -> Option<String>;
    fn usage_page(&self) -> Option<u16>;
    fn usage(&self) -> u16;
    fn interface_number(&self) -> i32;
}

pub fn create_backend() -> HidResult<self::Backend> {
    self::Backend::create()
}
