extern crate hidapi;
extern crate threadgroup;
extern crate core;

use hidapi::{HidApi, HidResult};
use std::sync::{Arc, Mutex};
use std::thread;

struct AnotherTestThing;

impl AnotherTestThing{
    fn do_something(test_thing: Arc<TestThing>){
        thread::spawn(move || {
            let test_clone1 = Arc::clone(&test_thing);
            let thread1 = thread::Builder::new().name("thread1".to_string()).spawn(move || {
                let hidapi = Mutex::lock(&test_clone1.hidapi).unwrap();
                let id = thread::current().id();
                println!("[{:?}]: Printing devices in separate thread:", id);
                for d in hidapi.devices() {
                    let dclone = &d.clone();
                    let product_string = dclone.product_string.clone();
                    let man_string = dclone.manufacturer_string.clone();
                    println!("[{:?}]: ---------------------", id);
                    println!("[{:?}]: vid: {:?}", id, dclone.vendor_id);
                    println!("[{:?}]: pid: {:?}", id, dclone.product_id);
                    println!("[{:?}]: prod_str: {:?}", id, &product_string.expect("Could not unwrap product_string"));
                    println!("[{:?}]: man_str: {:?}", id, &man_string.unwrap());
                    println!("[{:?}]: Opening device...", id);
                    let res = dclone.open_device(&hidapi);
                    match res {
                        Ok(d) => {
                            println!("[{:?}]: Successfully opened device: {:?}", id, d.get_product_string().unwrap());
                        },
                        Err(e) => {
                            eprintln!("[{:?}]: Could not opened device; continuing anyways!", id);
                            eprintln!("[{:?}]:\tError: {}", id, e);
                        }
                    }
                }
            }).unwrap();
            let test_clone2 = Arc::clone(&test_thing);
            let thread2 = thread::Builder::new().name("thread2".to_string()).spawn(move || {
                let hidapi = Mutex::lock(&test_clone2.hidapi).unwrap();
                let id = thread::current().id();
                println!("[{:?}]: Printing devices in separate thread:", id);
                for d in hidapi.devices() {
                    let dclone = &d.clone();
                    let product_string = dclone.product_string.clone();
                    let man_string = dclone.manufacturer_string.clone();
                    println!("[{:?}]: ---------------------", id);
                    println!("[{:?}]: vid: {:?}", id, dclone.vendor_id);
                    println!("[{:?}]: pid: {:?}", id, dclone.product_id);
                    println!("[{:?}]: prod_str: {:?}", id, &product_string.expect("Could not unwrap product_string"));
                    println!("[{:?}]: man_str: {:?}", id, &man_string.unwrap());
                    println!("[{:?}]: Opening device...", id);
                    let res = dclone.open_device(&hidapi);
                    match res {
                        Ok(d) => {
                            println!("[{:?}]: Successfully opened device: {:?}", id, d.get_product_string().unwrap());
                        },
                        Err(e) => {
                            eprintln!("[{:?}]: Could not opened device; continuing anyways!", id);
                            eprintln!("[{:?}]:\tError: {}", id, e);
                        }
                    }
                }
            }).unwrap();
            thread1.join().unwrap();
            thread2.join().unwrap();
        }).join().unwrap();
    }
}

struct TestThing{
    hidapi: Arc<Mutex<HidApi>>
}

impl TestThing{
    pub fn new() -> Option<TestThing>{
        let _hidapi = HidApi::new();
        match _hidapi{
            Ok(h) => {
                let harc = Arc::new(Mutex::new(h));
                Some(TestThing{ hidapi: harc })
            },
            Err(e) => {
                eprintln!("Error: {}", e);
                None
            }
        }
    }
}

fn main(){
    let test_thing = Arc::new(TestThing::new().unwrap());
    AnotherTestThing::do_something(test_thing);
}