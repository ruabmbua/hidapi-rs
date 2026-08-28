/****************************************************************************
    Copyright (c) 2015 Artyom Pavlov All Rights Reserved.

    This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
    It's also based on the Oleg Bulatov's work (https://github.com/dmage/co2mon)
****************************************************************************/

//! Opens a KIT MT 8057 CO2 detector and reads data from it. This
//! example will not work unless such an HID is plugged in to your system.

extern crate hidapi;

use hidapi::{HidApi, HidError};
use std::time::Duration;

use tokio::time::timeout;

// Reuse the code from the co2mon example so we're not writing the decoding and
// decrypting code multiple times.
#[allow(dead_code)]
#[path = "co2mon.rs"]
mod co2mon;

use co2mon::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), HidError> {
    let api = HidApi::new().expect("HID API object creation failed");
    let dev = api.open(DEV_VID, DEV_PID)?;
    dev.send_feature_report(&[0; PACKET_SIZE])?;

    if let Some(manufacturer) = dev.get_manufacturer_string()? {
        println!("Manufacurer:\t{manufacturer}");
    }
    if let Some(product) = dev.get_product_string()? {
        println!("Product:\t{product}");
    }
    if let Some(serial_number) = dev.get_serial_number_string()? {
        println!("Serial number:\t{serial_number}");
    }

    loop {
        let mut buf = [0; PACKET_SIZE];
        let n = timeout(
            Duration::from_millis(HID_TIMEOUT as u64),
            dev.async_read(&mut buf[..]),
        )
        .await
        .map_err(|_| invalid_data_err("timeout"))??;
        if n != PACKET_SIZE {
            let msg = format!("unexpected packet length: {n}/{PACKET_SIZE}");
            return Err(invalid_data_err(msg));
        }
        match decode_buf(buf) {
            CO2Result::Temperature(val) => println!("Temp:\t{:?}", val),
            CO2Result::Concentration(val) => println!("Conc:\t{:?}", val),
            CO2Result::Unknown(kind, val) => eprintln!("Unknown({kind:x}):\t{val}"),
            CO2Result::Error(msg) => {
                return Err(invalid_data_err(msg));
            }
        }
    }
}
