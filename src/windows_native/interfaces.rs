use std::ptr::{null, null_mut};
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_PropertyW, CR_BUFFER_SMALL, CR_SUCCESS};
use crate::ensure;
use crate::windows_native::error::{check_config, WinError, WinResult};
use crate::windows_native::hid::get_interface_guid;
use crate::windows_native::string::{U16Str, U16StringList};
use crate::windows_native::types::{DeviceProperty, PropertyKey};

pub struct Interface;

impl Interface {

    fn get_property_size<T: DeviceProperty>(interface: &U16Str, property_key: impl PropertyKey) -> WinResult<usize> {
        let mut property_type = 0;
        let mut len = 0;
        let cr = unsafe {
            CM_Get_Device_Interface_PropertyW(
                interface.as_ptr(),
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

    pub fn get_property<T: DeviceProperty>(interface: &U16Str, property_key: impl PropertyKey) -> WinResult<T> {
        let size = Self::get_property_size::<T>(interface, property_key)?;
        let mut property = T::create_sized(size);
        let mut property_type = 0;
        let mut len = size as u32;
        let cr = unsafe {
            CM_Get_Device_Interface_PropertyW(
                interface.as_ptr(),
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

    fn get_interface_list_length(interface: GUID) -> WinResult<usize> {
        let mut len = 0;
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut len,
                &interface,
                null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT)
        };
        check_config(cr, CR_SUCCESS)?;
        Ok(len as usize)
    }

    pub fn get_interface_list() -> WinResult<U16StringList> {
        let interface_class_guid = get_interface_guid();

        let mut device_interface_list = Vec::new();
        loop {
            device_interface_list.resize(Self::get_interface_list_length(interface_class_guid)?, 0);
            let cr = unsafe {
                CM_Get_Device_Interface_ListW(
                    &interface_class_guid,
                    null(),
                    device_interface_list.as_mut_ptr(),
                    device_interface_list.len() as u32,
                    CM_GET_DEVICE_INTERFACE_LIST_PRESENT
                )
            };
            if cr == CR_SUCCESS {
                return Ok(U16StringList(device_interface_list));
            }
            check_config(cr, CR_BUFFER_SMALL)?;
        }
    }
}

