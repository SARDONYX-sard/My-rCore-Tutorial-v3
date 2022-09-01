#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

#[allow(unreachable_code)]
#[no_mangle]
fn main() -> i32 {
    println!("---------------- store_fault start--------------------");
    println!("- store_fault/Into: Test store_fault, we will insert an invalid store operation...");
    println!("- store_fault/expect:Kernel should kill this application!");
    unsafe {
        core::ptr::null_mut::<u8>().write_volatile(0);
    }
    unreachable!();
    0
}
