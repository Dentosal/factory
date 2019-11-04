#[no_mangle]
pub extern "C" fn sum(a: u32, b: u32) -> u32 {
    a + b
}

#[no_mangle]
pub extern "C" fn print_int(a: u32) {
    println!("{}", a);
}
