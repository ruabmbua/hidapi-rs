//! The implementation which uses the the raw win32 api to perform operations

macro_rules! ensure {
    ($cond:expr, $result:expr) => {
        if !($cond) {
            return $result;
        }
    };
}

mod descriptor;
mod dev_node;
mod device_info;
mod error;
mod hid;
mod interfaces;
mod string;
mod types;
mod utils;

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::{
    ffi::CStr,
    fmt::{self, Debug},
};

use crate::windows_native::dev_node::DevNode;
use crate::windows_native::device_info::get_device_info;
use crate::windows_native::error::{check_boolean, Win32Error, WinError, WinResult};
use crate::windows_native::hid::{get_hid_attributes, PreparsedData};
use crate::windows_native::interfaces::Interface;
use crate::windows_native::string::{U16Str, U16String};
use crate::windows_native::types::{Handle, Overlapped};
use crate::{DeviceInfo, HidDeviceBackendBase, HidDeviceBackendWindows, HidError, HidResult};
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HidD_GetIndexedString, HidD_SetFeature, HidD_SetNumInputBuffers, HidD_SetOutputReport,
};
use windows_sys::Win32::Devices::Properties::{
    DEVPKEY_Device_ContainerId, DEVPKEY_Device_InstanceId,
};
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, TRUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::ResetEvent;
use windows_sys::Win32::System::IO::{CancelIoEx, DeviceIoControl};

const STRING_BUF_LEN: usize = 128;

pub struct HidApiBackend;
impl HidApiBackend {
    pub async fn get_hid_device_info_vector(vid: u16, pid: u16) -> HidResult<Vec<DeviceInfo>> {
        Ok(enumerate_devices(vid, pid)?)
    }

    pub fn open(vid: u16, pid: u16) -> HidResult<HidDevice> {
        open(vid, pid, None)
    }

    pub async fn open_serial(vid: u16, pid: u16, sn: &str) -> HidResult<HidDevice> {
        open(vid, pid, Some(sn))
    }

    pub fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
        open_path(device_path)
    }
}

/// Object for accessing HID device
pub struct HidDevice {
    device_handle: Handle,
    device_info: DeviceInfo,
    read_pending: Cell<bool>,
    blocking: Cell<bool>,
    read_state: RefCell<AsyncState>,
    write_state: RefCell<AsyncState>,
    feature_state: RefCell<AsyncState>,
}

struct AsyncState {
    overlapped: Box<Overlapped>,
    buffer: Vec<u8>,
}

impl AsyncState {
    fn new(report_size: usize) -> Self {
        Self {
            overlapped: Default::default(),
            buffer: vec![0u8; report_size],
        }
    }

    fn clear_buffer(&mut self) {
        self.buffer.fill(0)
    }

    fn fill_buffer(&mut self, data: &[u8]) {
        // Make sure the right number of bytes are passed to WriteFile. Windows
        // expects the number of bytes which are in the _longest_ report (plus
        // one for the report number) bytes even if the data is a report
        // which is shorter than that. Windows gives us this value in
        // caps.OutputReportByteLength. If a user passes in fewer bytes than this,
        // use cached temporary buffer which is the proper size.
        let data_size = data.len().min(self.buffer.len());
        self.buffer[..data_size].copy_from_slice(&data[..data_size]);
        if data_size < self.buffer.len() {
            self.buffer[data_size..].fill(0);
        }
    }

    fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    fn buffer_ptr(&mut self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }
}

impl Debug for HidDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HidDevice").finish()
    }
}

impl HidDeviceBackendBase for HidDevice {
    fn write(&self, data: &[u8]) -> HidResult<usize> {
        ensure!(!data.is_empty(), Err(HidError::InvalidZeroSizeData));
        let mut state = self.write_state.borrow_mut();
        state.fill_buffer(data);

        let res = unsafe {
            WriteFile(
                self.device_handle.as_raw(),
                state.buffer_ptr(),
                state.buffer_len() as u32,
                null_mut(),
                state.overlapped.as_raw(),
            )
        };

        if res != TRUE {
            let err = Win32Error::last();
            ensure!(err == Win32Error::IoPending, Err(err.into()));
            Ok(state
                .overlapped
                .get_result(&self.device_handle, Some(1000))?)
        } else {
            Ok(0)
        }
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        self.read_timeout(buf, if self.blocking.get() { -1 } else { 0 })
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        ensure!(!buf.is_empty(), Err(HidError::InvalidZeroSizeData));
        let mut bytes_read = 0;
        let mut io_runnig = false;
        let mut state = self.read_state.borrow_mut();

        if !self.read_pending.get() {
            self.read_pending.set(true);
            state.clear_buffer();
            let res = unsafe {
                ResetEvent(state.overlapped.event_handle());
                ReadFile(
                    self.device_handle.as_raw(),
                    state.buffer_ptr() as _,
                    state.buffer_len() as u32,
                    &mut bytes_read,
                    state.overlapped.as_raw(),
                )
            };
            if res != TRUE {
                let err = Win32Error::last();
                if err != Win32Error::IoPending {
                    unsafe { CancelIoEx(self.device_handle.as_raw(), state.overlapped.as_raw()) };
                    self.read_pending.set(false);
                    return Err(err.into());
                }
                io_runnig = true;
            }
        } else {
            io_runnig = true;
        }

        if io_runnig {
            let res = state
                .overlapped
                .get_result(&self.device_handle, u32::try_from(timeout).ok());
            bytes_read = match res {
                Ok(written) => written as u32,
                //There was no data this time. Return zero bytes available, but leave the Overlapped I/O running.
                Err(WinError::WaitTimedOut) => return Ok(0),
                Err(err) => {
                    self.read_pending.set(false);
                    return Err(err.into());
                }
            };
        }
        self.read_pending.set(false);

        let mut copy_len = 0;
        if bytes_read > 0 {
            // If report numbers aren't being used, but Windows sticks a report
            // number (0x0) on the beginning of the report anyway. To make this
            // work like the other platforms, and to make it work more like the
            // HID spec, we'll skip over this byte.
            if state.buffer[0] == 0x0 {
                bytes_read -= 1;
                copy_len = usize::min(bytes_read as usize, buf.len());
                buf[..copy_len].copy_from_slice(&state.buffer[1..(1 + copy_len)]);
            } else {
                copy_len = usize::min(bytes_read as usize, buf.len());
                buf[..copy_len].copy_from_slice(&state.buffer[0..copy_len]);
            }
        }
        Ok(copy_len)
    }

    fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        ensure!(!data.is_empty(), Err(HidError::InvalidZeroSizeData));
        let mut state = self.feature_state.borrow_mut();
        state.fill_buffer(data);

        check_boolean(unsafe {
            HidD_SetFeature(
                self.device_handle.as_raw(),
                state.buffer_ptr() as _,
                state.buffer_len() as u32,
            )
        })?;

        Ok(())
    }

    /// Set the first byte of `buf` to the 'Report ID' of the report to be read.
    /// Upon return, the first byte will still contain the Report ID, and the
    /// report data will start in `buf[1]`.
    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        #[allow(clippy::identity_op, clippy::double_parens)]
        const IOCTL_HID_GET_FEATURE: u32 = ((0x0000000b) << 16) | ((0) << 14) | ((100) << 2) | (2);
        ensure!(!buf.is_empty(), Err(HidError::InvalidZeroSizeData));
        let mut state = self.feature_state.borrow_mut();
        let mut bytes_returned = 0;

        let res = unsafe {
            ResetEvent(state.overlapped.event_handle());
            DeviceIoControl(
                self.device_handle.as_raw(),
                IOCTL_HID_GET_FEATURE,
                buf.as_mut_ptr() as _,
                buf.len() as u32,
                buf.as_mut_ptr() as _,
                buf.len() as u32,
                &mut bytes_returned,
                state.overlapped.as_raw(),
            )
        };
        if res != TRUE {
            let err = Win32Error::last();
            ensure!(err == Win32Error::IoPending, Err(err.into()))
        }

        bytes_returned = state.overlapped.get_result(&self.device_handle, None)? as u32;

        if buf[0] == 0x0 {
            bytes_returned += 1;
        }

        Ok(bytes_returned as usize)
    }

    fn send_output_report(&self, data: &[u8]) -> HidResult<()> {
        ensure!(!data.is_empty(), Err(HidError::InvalidZeroSizeData));
        let mut state = self.feature_state.borrow_mut();
        state.fill_buffer(data);

        check_boolean(unsafe {
            HidD_SetOutputReport(
                self.device_handle.as_raw(),
                state.buffer_ptr() as _,
                state.buffer_len() as u32,
            )
        })?;

        Ok(())
    }

    fn set_blocking_mode(&self, blocking: bool) -> HidResult<()> {
        self.blocking.set(blocking);
        Ok(())
    }

    fn get_manufacturer_string(&self) -> HidResult<Option<String>> {
        Ok(self.device_info.manufacturer_string().map(String::from))
    }

    fn get_product_string(&self) -> HidResult<Option<String>> {
        Ok(self.device_info.product_string().map(String::from))
    }

    fn get_serial_number_string(&self) -> HidResult<Option<String>> {
        Ok(self.device_info.serial_number().map(String::from))
    }

    fn get_indexed_string(&self, index: i32) -> HidResult<Option<String>> {
        let mut buf = [0u16; STRING_BUF_LEN];
        let res = unsafe {
            HidD_GetIndexedString(
                self.device_handle.as_raw(),
                index as u32,
                buf.as_mut_ptr() as _,
                STRING_BUF_LEN as u32,
            )
        };
        check_boolean(res)?;
        Ok(buf.split(|c| *c == 0).map(String::from_utf16_lossy).next())
    }

    fn get_device_info(&self) -> HidResult<DeviceInfo> {
        Ok(self.device_info.clone())
    }

    fn get_report_descriptor(&self, buf: &mut [u8]) -> HidResult<usize> {
        let desc = descriptor::get_descriptor(&PreparsedData::load(&self.device_handle)?)?;
        let size = buf.len().min(desc.len());
        buf[..size].copy_from_slice(&desc[..size]);
        Ok(size)
    }
}

impl HidDeviceBackendWindows for HidDevice {
    fn get_container_id(&self) -> HidResult<GUID> {
        let path =
            U16String::try_from(self.device_info.path()).expect("device path is not valid unicode");

        let device_id: U16String = Interface::get_property(&path, DEVPKEY_Device_InstanceId)?;

        let dev_node = DevNode::from_device_id(&device_id)?;
        let guid = dev_node.get_property(DEVPKEY_Device_ContainerId)?;
        Ok(guid)
    }
}

impl Drop for HidDevice {
    fn drop(&mut self) {
        unsafe {
            for state in [
                &mut self.read_state,
                &mut self.write_state,
                &mut self.feature_state,
            ] {
                let mut state = state.borrow_mut();
                if CancelIoEx(self.device_handle.as_raw(), state.overlapped.as_raw()) > 0 {
                    _ = state.overlapped.get_result(&self.device_handle, None);
                }
            }
        }
    }
}

fn enumerate_devices(vendor_id: u16, product_id: u16) -> WinResult<Vec<DeviceInfo>> {
    Ok(Interface::get_interface_list()?
        .iter()
        .filter_map(|device_interface| {
            let device_handle = open_device(device_interface, false).ok()?;
            let attrib = get_hid_attributes(&device_handle);
            ((vendor_id == 0 || attrib.VendorID == vendor_id)
                && (product_id == 0 || attrib.ProductID == product_id))
                .then(|| get_device_info(device_interface, &device_handle))
        })
        .collect())
}

fn open_device(path: &U16Str, open_rw: bool) -> WinResult<Handle> {
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            match open_rw {
                true => GENERIC_WRITE | GENERIC_READ,
                false => 0,
            },
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            0,
        )
    };
    ensure!(
        handle != INVALID_HANDLE_VALUE,
        Err(Win32Error::last().into())
    );
    Ok(Handle::from_raw(handle))
}

fn open(vid: u16, pid: u16, sn: Option<&str>) -> HidResult<HidDevice> {
    let dev = enumerate_devices(vid, pid)?
        .into_iter()
        .filter(|dev| dev.vendor_id == vid && dev.product_id == pid)
        .find(|dev| sn.map_or(true, |sn| dev.serial_number().is_some_and(|n| sn == n)))
        .ok_or(HidError::HidApiErrorEmpty)?;
    open_path(dev.path())
}

fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
    let device_path = U16String::try_from(device_path).unwrap();
    let handle = open_device(&device_path, true)
        // System devices, such as keyboards and mice, cannot be opened in
        // read-write mode, because the system takes exclusive control over
        // them.  This is to prevent keyloggers.  However, feature reports
        // can still be sent and received.  Retry opening the device, but
        // without read/write access.
        .or_else(|_| open_device(&device_path, false))?;
    check_boolean(unsafe { HidD_SetNumInputBuffers(handle.as_raw(), 64) })?;
    let caps = PreparsedData::load(&handle)?.get_caps()?;
    let device_info = get_device_info(&device_path, &handle);
    let dev = HidDevice {
        device_handle: handle,
        blocking: Cell::new(true),
        read_pending: Cell::new(false),
        read_state: RefCell::new(AsyncState::new(caps.InputReportByteLength as usize)),
        write_state: RefCell::new(AsyncState::new(caps.OutputReportByteLength as usize)),
        feature_state: RefCell::new(AsyncState::new(caps.FeatureReportByteLength as usize)),
        device_info,
    };

    Ok(dev)
}
