/****************************************************************************
    Copyright (c) 2015 Osspial All Rights Reserved.

    This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
****************************************************************************/

//! Prints out a list of HID devices

extern crate hidapi;

use hidapi::HidApi;

fn main() {
    println!("Printing all available hid devices:");

    match HidApi::new() {
        Ok(api) => {
            for device in api.device_list() {
                println!(
                    "VID: {:04x}, PID: {:04x}, Serial: {}, Product name: {}, Interface: {}",
                    device.vendor_id(),
                    device.product_id(),
                    device.serial_number().unwrap_or("<COULD NOT FETCH>"),
                    device.product_string().unwrap_or("<COULD NOT FETCH>"),
                    device.interface_number()
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
