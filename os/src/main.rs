#![feature(panic_info_message)]
#![no_main]
#![no_std]

#[macro_use]
mod console;
mod lang_items;
mod sbi;
mod sync;

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));

#[no_mangle]
fn rust_main() -> ! {
    clear_bss();
    println!("Hello World!");
    panic!("Shutdown machine!");
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }

    // Dereference the memory address of sbss and ebss, and write 0
    // like(*sbss = 0, *ebss = 0)
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}
