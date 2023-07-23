// **************************************************************************
// Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi-rs by Osspial
// **************************************************************************

use libc::wchar_t;
use std::error::Error;
use std::fmt::{Display, Formatter, Result};

use crate::DeviceInfo;

#[derive(Debug)]
pub enum HidError {
    HidApiError {
        message: String,
    },
    HidApiErrorEmpty,
    FromWideCharError {
        wide_char: wchar_t,
    },
    InitializationError,
    InvalidZeroSizeData,
    IncompleteSendError {
        sent: usize,
        all: usize,
    },
    SetBlockingModeError {
        mode: &'static str,
    },
    OpenHidDeviceWithDeviceInfoError {
        device_info: Box<DeviceInfo>,
    },
    /// An IO error or a system error that can be represented as such
    IoError {
        error: std::io::Error,
    },
}

impl Display for HidError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            HidError::HidApiError { message } => write!(f, "hidapi error: {}", message),
            HidError::HidApiErrorEmpty => write!(f, "hidapi error: (could not get error message)"),
            HidError::FromWideCharError { wide_char } => {
                write!(f, "failed converting {:#X} to rust char", wide_char)
            }
            HidError::InitializationError => {
                write!(f, "Failed to initialize hidapi")
            }
            HidError::InvalidZeroSizeData => write!(f, "Invalid data: size can not be 0"),
            HidError::IncompleteSendError { sent, all } => write!(
                f,
                "Failed to send all data: only sent {} out of {} bytes",
                sent, all
            ),
            HidError::SetBlockingModeError { mode } => {
                write!(f, "Can not set blocking mode to '{}'", mode)
            }
            HidError::OpenHidDeviceWithDeviceInfoError { device_info } => {
                write!(f, "Can not open hid device with: {:?}", *device_info)
            }
            HidError::IoError { error } => {
                write!(f, "{error}")
            }
        }
    }
}

impl Error for HidError {}

impl From<std::io::Error> for HidError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError { error: e }
    }
}

#[cfg(all(feature = "linux-native", target_os = "linux"))]
impl From<nix::errno::Errno> for HidError {
    fn from(e: nix::errno::Errno) -> Self {
        Self::IoError { error: e.into() }
    }
}
