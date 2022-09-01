#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use core::arch::asm;

#[allow(unreachable_code)]
#[no_mangle]
fn main() -> i32 {
    println!("------------------ priv_inst start -------------------");
    println!("- priv_inst/info: Try to execute privileged instruction in U Mode");
    println!("- priv_inst/expect:Kernel should kill this application!");
    unsafe {
        asm!("sret");
    }
    unreachable!();
    0
}
