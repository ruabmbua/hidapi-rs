use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{HIDD_ATTRIBUTES, HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetPreparsedData, HIDP_CAPS, HidP_GetCaps, HIDP_STATUS_SUCCESS};
use crate::ensure;
use crate::windows_native::error::{check_boolean, WinError, WinResult};
use crate::windows_native::types::Handle;

pub fn get_interface_guid() -> GUID {
    unsafe {
        let mut guid = zeroed();
        HidD_GetHidGuid(&mut guid);
        guid
    }
}

pub fn get_hid_attributes(handle: &Handle) -> HIDD_ATTRIBUTES {
    unsafe {
        let mut attrib = HIDD_ATTRIBUTES {
            Size: size_of::<HIDD_ATTRIBUTES>() as u32,
            ..zeroed()
        };
        HidD_GetAttributes(handle.as_raw(), &mut attrib);
        attrib
    }
}

#[repr(transparent)]
pub struct PreparsedData(isize);

impl Drop for PreparsedData {
    fn drop(&mut self) {
        unsafe { HidD_FreePreparsedData(self.0); }
    }
}

impl PreparsedData {

    pub fn load(handle: &Handle) -> WinResult<Self> {
        let mut pp_data = 0;
        check_boolean(unsafe { HidD_GetPreparsedData(handle.as_raw(), &mut pp_data) })?;
        ensure!(pp_data != 0, Err(WinError::InvalidPreparsedData));
        Ok(Self(pp_data))
    }

    #[allow(dead_code)]
    pub fn as_ptr(&self) -> *const c_void {
        self.0 as _
    }

    pub fn get_caps(&self) -> WinResult<HIDP_CAPS> {
        unsafe {
            let mut caps = zeroed();
            let r = HidP_GetCaps(self.0, &mut caps);
            ensure!(r == HIDP_STATUS_SUCCESS, Err(WinError::InvalidPreparsedData));
            Ok(caps)
        }
    }

}
