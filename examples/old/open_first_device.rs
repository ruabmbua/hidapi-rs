/****************************************************************************
    Copyright (c) 2018 Roland Ruckerbauer All Rights Reserved.

    This file is part of hidapi-rs, based on hidapi-rs by Osspial
****************************************************************************/

//! Opens the first hid device it can find, and reads data in a blocking fashion
//! from it in an endless loop.

extern crate failure;
extern crate hidapi;

use failure::Error;
use hidapi::HidApi;

fn main() {
    fn run() -> Result<(), Error> {
        let hidapi = HidApi::new()?;

        let device_info = hidapi
            .devices()
            .iter()
            .next()
            .expect("No devices are available!")
            .clone();

        println!("Opening device:\n {:#?}\n", device_info);

        let device = device_info.open_device(&hidapi)?;

        let mut buf = vec![0; 64];

        println!("Reading data from device ...\n");

        loop {
            let len = device.read(&mut buf)?;
            println!("{:?}", &buf[..len]);
        }

        Ok(())
    }

    if let Err(e) = run() {
        eprintln!("Error: {}", e);
    }
}
