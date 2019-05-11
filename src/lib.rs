// **************************************************************************
// Copyright (c) 2015 Osspial All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
// *************************************************************************

//! This crate provides a rust abstraction over the features of the C library
//! hidapi by [signal11](https://github.com/signal11/hidapi).
//!
//! # Usage
//!
//! This crate is [on crates.io](https://crates.io/crates/hidapi) and can be
//! used by adding `hidapi` to the dependencies in your project's `Cargo.toml`.
//!
//! # Example
//!
//! ```rust,no_run
//! extern crate hidapi;
//!
//! use hidapi::HidApi;
//!
//! fn main() {
//!     println!("Printing all available hid devices:");
//!
//!     match HidApi::new() {
//!         Ok(api) => {
//!             for device in api.devices() {
//!                 println!("{:#?}", device);
//!             }
//!         },
//!         Err(e) => {
//!             eprintln!("Error: {}", e);
//!         },
//!     }
//! }
//! ```

#[macro_use]
extern crate failure_derive;
extern crate failure;

mod backend;
mod error;
mod parser;
