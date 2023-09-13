//! The extra behaviour for Windows

use std::ptr::addr_of_mut;
use windows_sys::core::GUID;

use super::HidDevice;
use crate::{ffi, HidDeviceBackendBase, HidDeviceBackendWindows, HidResult};

impl HidDeviceBackendWindows for HidDevice {
    fn get_container_id(&self) -> HidResult<GUID> {
        let mut container_id: GUID = unsafe { std::mem::zeroed() };

        let res = unsafe {
            ffi::windows::hid_winapi_get_container_id(self._hid_device, addr_of_mut!(container_id))
        };

        if res == -1 {
            match self.check_error() {
                Ok(err) => Err(err),
                Err(err) => Err(err),
            }
        } else {
            Ok(container_id)
        }
    }
}
