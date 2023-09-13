use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Foundation::*;
use crate::HidError;

pub type WinResult<T> = Result<T, WinError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WinError {
    Config(CONFIGRET),
    Win32(Win32Error),
    BufferTooSmall,
    InvalidDeviceId,
    InvalidDeviceNode,
    NoSuchValue,
    WrongPropertyDataType,
    UnexpectedReturnSize,
    InvalidPreparsedData,
    WaitTimedOut
}

impl WinError {

    pub fn last() -> Self {
        Self::from(Win32Error::last())
    }

}

impl From<WinError> for HidError {
    fn from(value: WinError) -> Self {
        match value {
            WinError::Win32(Win32Error::Generic(err)) => HidError::IoError { error: std::io::Error::from_raw_os_error(err as _) },
            err => HidError::HidApiError { message: format!("WinError: {:?}", err)}
        }
    }
}

fn config_to_error(ret: CONFIGRET) -> WinError {
    match ret {
        CR_BUFFER_SMALL => WinError::BufferTooSmall,
        CR_INVALID_DEVICE_ID => WinError::InvalidDeviceId,
        CR_INVALID_DEVNODE => WinError::InvalidDeviceNode,
        CR_NO_SUCH_VALUE => WinError::NoSuchValue,
        ret => WinError::Config(ret)
    }
}

pub fn check_config(ret: CONFIGRET, expected: CONFIGRET) -> WinResult<()>  {
    if ret == expected {
        Ok(())
    } else {
        Err(config_to_error(ret))
    }
}

pub fn check_boolean(ret: BOOLEAN) -> WinResult<()> {
    if ret == 0 {
        Err(Win32Error::last().into())
    } else {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Win32Error {
    Generic(WIN32_ERROR),
    Success,
    IoPending,
    WaitTimedOut
}

impl Win32Error {

    pub fn last() -> Self {
        match unsafe { GetLastError() }  {
            NO_ERROR => Self::Success,
            ERROR_IO_PENDING => Self::IoPending,
            ERROR_IO_INCOMPLETE | WAIT_TIMEOUT => Self::WaitTimedOut,
            code => Self::Generic(code)
        }
    }

    //pub fn is_error(self) -> bool {
    //    !matches!(self, Win32Error::Success | Win32Error::IoPending)
    //}

}

impl From<Win32Error> for WinError {
    fn from(value: Win32Error) -> Self {
        match value {
            Win32Error::WaitTimedOut => Self::WaitTimedOut,
            err => Self::Win32(err)
        }
    }
}

impl From<Win32Error> for HidError {
    fn from(value: Win32Error) -> Self {
        HidError::from(WinError::from(value))
    }
}