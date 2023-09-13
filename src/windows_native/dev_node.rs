use std::ptr::null_mut;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{CM_Get_DevNode_PropertyW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CR_BUFFER_SMALL, CR_SUCCESS};
use crate::ensure;
use crate::windows_native::error::{check_config, WinError, WinResult};
use crate::windows_native::string::U16Str;
use crate::windows_native::types::{DeviceProperty, PropertyKey};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DevNode(u32);

impl DevNode {

    pub fn from_device_id(device_id: &U16Str) -> WinResult<Self> {
        let mut node = 0;
        let cr = unsafe {
            CM_Locate_DevNodeW(&mut node, device_id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL)
        };
        check_config(cr, CR_SUCCESS)?;
        Ok(Self(node))
    }

    pub fn parent(self) -> WinResult<Self> {
        let mut parent = 0;
        let cr = unsafe { CM_Get_Parent(&mut parent, self.0, 0) };
        check_config(cr, CR_SUCCESS)?;
        Ok(Self(parent))
    }

    fn get_property_size<T: DeviceProperty>(self, property_key: impl PropertyKey) -> WinResult<usize> {
        let mut property_type = 0;
        let mut len = 0;
        let cr = unsafe {
            CM_Get_DevNode_PropertyW(
                self.0,
                property_key.as_ptr(),
                &mut property_type,
                null_mut(),
                &mut len,
                0
            )
        };
        check_config(cr, CR_BUFFER_SMALL)?;
        ensure!(property_type == T::TYPE, Err(WinError::WrongPropertyDataType));
        Ok(len as usize)
    }

    pub fn get_property<T: DeviceProperty>(self, property_key: impl PropertyKey) -> WinResult<T> {
        let size = self.get_property_size::<T>(property_key)?;
        let mut property = T::create_sized(size);
        let mut property_type = 0;
        let mut len = size as u32;
        let cr = unsafe {
            CM_Get_DevNode_PropertyW(
                self.0,
                property_key.as_ptr(),
                &mut property_type,
                property.as_ptr_mut(),
                &mut len,
                0
            )
        };
        check_config(cr, CR_SUCCESS)?;
        ensure!(size == len as usize, Err(WinError::UnexpectedReturnSize));
        property.validate();
        Ok(property)
    }

}

