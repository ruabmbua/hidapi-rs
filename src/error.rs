// **************************************************************************
// Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi-rs by Osspial
// **************************************************************************

use super::DeviceInfo;
use libc::wchar_t;
use std::error::Error;
use std::fmt::{Display, Formatter, Result};

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
                write!(f, "Failed to initialize hidapi (maybe initialized before?)")
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
        }
    }
}

impl Error for HidError {}
