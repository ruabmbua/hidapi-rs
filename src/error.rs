// **************************************************************************
// Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi-rs by Osspial
// **************************************************************************

use failure::{Compat, Error};
#[cfg(not(feature = "linux-rust-hidraw"))]
use ::backend::libc::wchar_t;

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

    #[cfg(not(feature = "linux-rust-hidraw"))]
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
}