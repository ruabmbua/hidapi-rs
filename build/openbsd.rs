pub fn compile() {
    pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi");
    println!("cargo:rustc-cfg=libusb");
    println!("cargo:rustc-cfg=hidapi");
}
