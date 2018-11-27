// **************************************************************************
// Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi-rs by Osspial
// **************************************************************************

#[cfg(feature = "linux-rust-hidraw")]
pub mod linux_hidraw;

#[cfg(feature = "linux-rust-hidraw")]
pub use self::linux_hidraw::HidrawBackend as Backend; 


#[cfg(not(feature = "linux-rust-hidraw"))]
pub mod hidapi;

#[cfg(not(feature = "linux-rust-hidraw"))]
pub use hidapi::HidapiBackend as Backend;