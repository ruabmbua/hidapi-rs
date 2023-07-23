# hidapi [![Version](https://img.shields.io/crates/v/hidapi.svg)](https://crates.io/crates/hidapi) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Osspial/hidapi-rs/blob/master/LICENSE.txt) [![Documentation](https://docs.rs/hidapi/badge.svg)](https://docs.rs/hidapi) [![Chat](https://img.shields.io/badge/discord-devroom-blue.svg)](https://discordapp.com/invite/3ahhJGN)

This crate provides a rust abstraction over the features of the C library
[hidapi](https://github.com/libusb/hidapi). Based off of
[hidapi-rs](https://github.com/Osspial/hidapi-rs) by Osspial.

# Usage

This crate is on [crates.io](https://crates.io/crates/hidapi) and can be
used by adding `hidapi` to the dependencies in your project's `Cargo.toml`.

# Example

```rust
extern crate hidapi;

let api = hidapi::HidApi::new().unwrap();
// Print out information about all connected devices
for device in api.device_list() {
    println!("{:#?}", device);
}

// Connect to device using its VID and PID
let (VID, PID) = (0x0123, 0x3456);
let device = api.open(VID, PID).unwrap();

// Read data from device
let mut buf = [0u8; 8];
let res = device.read(&mut buf[..]).unwrap();
println!("Read: {:?}", &buf[..res]);

// Write data to device
let buf = [0u8, 1, 2, 3, 4];
let res = device.write(&buf).unwrap();
println!("Wrote: {:?} byte(s)", res);
```

# Documentation
Available at [docs.rs](https://docs.rs/hidapi).
