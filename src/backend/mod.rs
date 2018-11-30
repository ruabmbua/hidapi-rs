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

use error::{HidError, HidResult};
use std::io::{Read, Write};

pub trait ApiBackend
where
    Self: Sized,
    Self::Device: ApiDevice,
    Self::DeviceInfo: ApiDeviceInfo,
    Self::DeviceInfoIter: Iterator<Item = Self::DeviceInfo>,
    Self::Device: Write,
{
    type Device;
    type DeviceInfo;
    type DeviceInfoIter;

    fn create() -> HidResult<Self>;
    fn open_device(&self, vid: u16, pid: u16) -> HidResult<Self::Device>;
    fn enumerate(&mut self) -> HidResult<Self::DeviceInfoIter>;
}

pub trait ApiDevice: Write {}

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
