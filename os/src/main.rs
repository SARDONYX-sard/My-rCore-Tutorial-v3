//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`syscall`]: System call handling and implementation
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`batch::run_next_app()`] and for the first time go to
//! userspace.

#![deny(missing_docs)]
#![deny(warnings)]
#![feature(panic_info_message)]
#![no_main]
#![no_std]

use core::arch::global_asm;

#[cfg(feature = "board_qemu")]
#[path = "boards/qemu.rs"]
mod board;

pub mod batch;
#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod sbi;
mod sync;
pub mod syscall;
mod task;
pub mod trap;

global_asm!(include_str!("entry.asm"));
// The binary image file of the user's application
// created by os/build.rs(the ELF format executable file minus the metadata. previous)
// is linked to the kernel as a kernel data segment.
global_asm!(include_str!("link_app.S")); //

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }

    // Dereference the memory address of sbss and ebss, and write 0
    // like(*sbss = 0, *ebss = 0)
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

#[no_mangle]
fn rust_main() -> ! {
    clear_bss();
    println!("Hello World!");
    panic!("Shutdown machine!");
}
