// #[link(name = "example", kind = "static")]
// extern "C" {
//     fn sum(a: u32, b: u32) -> u32;
// }

// fn main() {
//     println!("1 + 2 = {}", unsafe { sum(1, 2) } );
// }

#[no_mangle]
pub extern "C" fn sum(a: u32, b: u32) -> u32 {
    a + b
}

#[no_mangle]
pub extern "C" fn print_int(a: u32) {
    println!("{}", a);
}
