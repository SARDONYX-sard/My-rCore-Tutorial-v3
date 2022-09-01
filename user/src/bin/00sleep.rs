#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

// #[no_mangle]
// pub fn main() -> i32 {
//     println!("Hello world from user mode program!");
//     0
// }

use user_lib::{get_time, yield_};

#[no_mangle]
fn main() -> i32 {
    println!("-------------------- sleep start ----------------------");
    let current_timer = get_time();
    let wait_for = current_timer + 3000;
    println!("- sleep/current timer: {}ms", current_timer);
    println!("- sleep/wait_for: {}ms", wait_for);
    while get_time() < wait_for {
        yield_();
    }
    println!("Test sleep OK!");
    println!("-------------------- sleep end -------------------------");
    0
}
