use std::fs::read_to_string;
use crate::windows_native::descriptor::get_descriptor_ptr;

#[test]
fn test_01() {
    execute_testcase("045E_02FF_0005_0001");
}
#[test]
fn test_02() {
    execute_testcase("046A_0011_0006_0001");
}
#[test]
fn test_03() {
    execute_testcase("046D_0A37_0001_000C");
}
#[test]
fn test_04() {
    execute_testcase("046D_B010_0001_000C");
}
#[test]
fn test_05() {
    execute_testcase("046D_B010_0001_FF00");
}
#[test]
fn test_06() {
    execute_testcase("046D_B010_0002_0001");
}
#[test]
fn test_07() {
    execute_testcase("046D_B010_0002_FF00");
}
#[test]
fn test_08() {
    execute_testcase("046D_B010_0006_0001");
}
#[test]
fn test_09() {
    execute_testcase("046D_C077_0002_0001");
}
#[test]
fn test_10() {
    execute_testcase("046D_C283_0004_0001");
}
#[test]
fn test_11() {
    execute_testcase("046D_C52F_0001_000C");
}
#[test]
fn test_12() {
    execute_testcase("046D_C52F_0001_FF00");
}
#[test]
fn test_13() {
    execute_testcase("046D_C52F_0002_0001");
}
#[test]
fn test_14() {
    execute_testcase("046D_C52F_0002_FF00");
}
#[test]
fn test_15() {
    execute_testcase("046D_C534_0001_000C");
}
#[test]
fn test_16() {
    execute_testcase("046D_C534_0001_FF00");
}
#[test]
fn test_17() {
    execute_testcase("046D_C534_0002_0001");
}
#[test]
fn test_18() {
    execute_testcase("046D_C534_0002_FF00");
}
#[test]
fn test_19() {
    execute_testcase("046D_C534_0006_0001");
}
#[test]
fn test_20() {
    execute_testcase("046D_C534_0080_0001");
}
#[test]
fn test_21() {
    execute_testcase("047F_C056_0001_000C");
}
#[test]
fn test_22() {
    execute_testcase("047F_C056_0003_FFA0");
}
#[test]
fn test_23() {
    execute_testcase("047F_C056_0005_000B");
}
#[test]
fn test_24() {
    execute_testcase("17CC_1130_0000_FF01");
}


fn execute_testcase(filename: &str) {
    let source_path = format!("./tests/pp_data/{filename}.pp_data");
    let expected_path = format!("./tests/pp_data/{filename}.expected");
    println!("Testing: {:?} <-> {:?}", source_path, expected_path);
    let pp_data = decode_hex(&read_to_string(&source_path).unwrap());
    let expected_descriptor = decode_hex(&read_to_string(&expected_path).unwrap());
    let constructed_descriptor = unsafe { get_descriptor_ptr(pp_data.as_ptr() as _) }.unwrap();
    assert_eq!(constructed_descriptor, expected_descriptor);
}

fn decode_hex(hex: &str) -> Vec<u8> {
    hex
        .lines()
        .flat_map(|line| line
            .split(',')
            .map(|hex| hex.trim())
            .filter(|hex| !hex.is_empty())
            .map(|hex| hex.strip_prefix("0x").unwrap())
            .map(|hex| u8::from_str_radix(hex, 16).unwrap()))
        .collect()
}
