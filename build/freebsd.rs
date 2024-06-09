pub fn compile() {
    pkg_config::probe_library("hidapi").expect("Unable to find hidapi");
    println!("cargo:rustc-cfg=libusb");
    println!("cargo:rustc-cfg=hidapi");
}
