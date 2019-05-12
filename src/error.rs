// **************************************************************************
// Copyright (c) 2019 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs
// **************************************************************************

use cfg_if::cfg_if;
use failure::{Compat, Error};
#[cfg(any(
    feature = "linux-static-hidraw",
    feature = "linux-static-libusb",
    feature = "linux-shared-hidraw",
    feature = "linux-shared-libusb"
))]
use libc::wchar_t;

pub type HidResult<T> = Result<T, HidError>;

#[derive(Debug, Fail)]
pub enum HidError {
    #[fail(display = "hidapi error: {}", message)]
    HidApiError { message: String },

    #[fail(
        display = "hidapi error: (could not get error message), caused by: {}",
        cause
    )]
    HidApiErrorEmptyWithCause {
        #[cause]
        cause: Compat<Error>,
    },

    #[fail(display = "hidapi error: (could not get error message)")]
    HidApiErrorEmpty,

    #[cfg(any(
        feature = "linux-static-hidraw",
        feature = "linux-static-libusb",
        feature = "linux-shared-hidraw",
        feature = "linux-shared-libusb"
    ))]
    #[fail(display = "failed converting {:#X} to rust char", wide_char)]
    FromWideCharError { wide_char: wchar_t },

    #[fail(display = "Failed to initialize hidapi (maybe initialized before?)")]
    InitializationError,

    #[fail(display = "Failed opening hid device")]
    OpenHidDeviceError,

    #[fail(display = "Invalid data: size can not be 0")]
    InvalidZeroSizeData,

    #[fail(
        display = "Failed to send all data: only sent {} out of {} bytes",
        sent, all
    )]
    IncompleteSendError { sent: usize, all: usize },

    #[fail(display = "Can not set blocking mode to '{}'", mode)]
    SetBlockingModeError { mode: &'static str },

    #[cfg(feature = "linux-rust-hidraw")]
    #[fail(display = "Udev error: {}", udev_e)]
    UdevError { udev_e: libudev::Error },

    #[cfg(feature = "linux-rust-hidraw")]
    #[fail(display = "Nix error: {}", nix_e)]
    NixError { nix_e: nix::Error },

    #[cfg(feature = "linux-rust-hidraw")]
    #[fail(display = "NulError: {}", nul_e)]
    NulError { nul_e: std::ffi::NulError },

    #[cfg(feature = "linux-rust-hidraw")]
    #[fail(display = "FromBytesWithNulError: {}", nul_e)]
    FromBytesWithNulError {
        nul_e: std::ffi::FromBytesWithNulError,
    },
}

pub trait ResultExt<T> {
    /// Convert any Result<T, E> into Result<T, HidError {E}>
    fn convert(self) -> Result<T, HidError>;
}

cfg_if! {
    if #[cfg(feature = "linux-rust-hidraw")] {
        impl<T> ResultExt<T> for Result<T, libudev::Error> {
            fn convert(self) -> Result<T, HidError> {
                self.map_err(|udev_e| HidError::UdevError { udev_e })
            }
        }
        impl<T> ResultExt<T> for Result<T, nix::Error> {
            fn convert(self) -> Result<T, HidError> {
                self.map_err(|nix_e| HidError::NixError { nix_e })
            }
        }
        impl<T> ResultExt<T> for Result<T, std::ffi::NulError> {
            fn convert(self) -> Result<T, HidError> {
                self.map_err(|nul_e| HidError::NulError { nul_e })
            }
        }
        impl<T> ResultExt<T> for Result<T, std::ffi::FromBytesWithNulError> {
            fn convert(self) -> Result<T, HidError> {
                self.map_err(|nul_e| HidError::FromBytesWithNulError { nul_e })
            }
        }
    }
}
