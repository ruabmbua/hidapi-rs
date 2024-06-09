/****************************************************************************
    Copyright (c) 2015 Artyom Pavlov All Rights Reserved.

    This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
    It's also based on the Oleg Bulatov's work (https://github.com/dmage/co2mon)
****************************************************************************/

//! Opens a KIT MT 8057 CO2 detector and reads data from it. This
//! example will not work unless such device is plugged into your system.

use hidapi::{HidApi, HidError};
use std::io;

const CODE_TEMPERATURE: u8 = 0x42;
const CODE_CONCENTRATION: u8 = 0x50;
const HID_TIMEOUT: i32 = 5000;
const DEV_VID: u16 = 0x04d9;
const DEV_PID: u16 = 0xa052;
const PACKET_SIZE: usize = 8;

type Packet = [u8; PACKET_SIZE];

enum CO2Result {
    Temperature(f32),
    Concentration(u16),
    Unknown(u8, u16),
    Error(&'static str),
}

fn decode_temperature(value: u16) -> f32 {
    (value as f32) * 0.0625 - 273.15
}

fn decrypt(buf: Packet) -> Packet {
    let mut res: [u8; PACKET_SIZE] = [
        (buf[3] << 5) | (buf[2] >> 3),
        (buf[2] << 5) | (buf[4] >> 3),
        (buf[4] << 5) | (buf[0] >> 3),
        (buf[0] << 5) | (buf[7] >> 3),
        (buf[7] << 5) | (buf[1] >> 3),
        (buf[1] << 5) | (buf[6] >> 3),
        (buf[6] << 5) | (buf[5] >> 3),
        (buf[5] << 5) | (buf[3] >> 3),
    ];

    let magic_word = b"Htemp99e";
    for i in 0..PACKET_SIZE {
        let sub_val: u8 = (magic_word[i] << 4) | (magic_word[i] >> 4);
        res[i] = u8::wrapping_sub(res[i], sub_val);
    }

    res
}

fn decode_buf(buf: Packet) -> CO2Result {
    // Do we need to decrypt the data?
    let res = if buf[4] == 0x0d { buf } else { decrypt(buf) };

    let kind = res[0];
    let val = u16::from_be_bytes(res[1..3].try_into().unwrap());
    let checksum = res[3];
    let tail = res[4];

    if tail != 0x0d {
        return CO2Result::Error("Unexpected data (data[4] != 0x0d)");
    }
    let checksum_calc = res[0].wrapping_add(res[1]).wrapping_add(res[2]);
    if checksum != checksum_calc {
        return CO2Result::Error("Checksum error");
    }

    match kind {
        CODE_TEMPERATURE => CO2Result::Temperature(decode_temperature(val)),
        CODE_CONCENTRATION => {
            if val > 3000 {
                CO2Result::Error("Concentration bigger than 3000 (uninitialized device?)")
            } else {
                CO2Result::Concentration(val)
            }
        }
        _ => CO2Result::Unknown(kind, val),
    }
}

fn invalid_data_err(msg: impl Into<String>) -> HidError {
    HidError::IoError {
        error: io::Error::new(io::ErrorKind::InvalidData, msg.into()),
    }
}

fn main() -> Result<(), HidError> {
    let api = HidApi::new()?;
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

    let mut buf = [0; PACKET_SIZE];
    loop {
        let n = dev.read_timeout(&mut buf[..], HID_TIMEOUT)?;
        if n != PACKET_SIZE {
            let msg = format!("unexpected packet length: {n}/{PACKET_SIZE}");
            return Err(invalid_data_err(msg));
        }
        match decode_buf(buf) {
            CO2Result::Temperature(val) => println!("Temp:\t{val}"),
            CO2Result::Concentration(val) => println!("Conc:\t{val}"),
            CO2Result::Unknown(kind, val) => eprintln!("Unknown({kind}):\t{val}"),
            CO2Result::Error(msg) => {
                return Err(invalid_data_err(msg));
            }
        }
    }
}
