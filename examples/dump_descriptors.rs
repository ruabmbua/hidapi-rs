use hidapi::HidApi;

fn main() {
    println!("Printing all available hid devices:");

    match HidApi::new() {
        Ok(api) => {
            for device in api.device_list() {
                println!("  {} (Interface {}):",
                    match device.product_string() {
                        Some(s) => s,
                        _ => "<COULD NOT FETCH>",
                    },
                    device.interface_number()
                );
                let mut descriptor = vec![0u8; 2048];
                match device.open_device(&api).and_then(|dev| dev.get_report_descriptor(&mut descriptor)) {
                    Ok(length) => println!("    {:?}", &mut descriptor[..length]),
                    Err(err) => println!("    Failed to retrieve descriptor ({:?})", err)
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}