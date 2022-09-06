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
#![feature(alloc_error_handler)]
#![no_main]
#![no_std]

extern crate alloc;

#[macro_use]
extern crate bitflags;

use core::arch::global_asm;

#[cfg(feature = "board_qemu")]
#[path = "boards/qemu.rs"]
mod board;

// pub mod batch;
#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod mm;
mod sbi;
mod sync;
pub mod syscall;
mod task;
mod timer;
pub mod trap;

global_asm!(include_str!("entry.asm"));
// The binary image file of the user's application
// created by os/build.rs(the ELF format executable file minus the metadata. previous)
// is linked to the kernel as a kernel data segment.
global_asm!(include_str!("link_app.S")); //

/// clear BSS segment
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }

    // Dereference the memory address of sbss and ebss, and write 0
    // like(*sbss = 0, *ebss = 0)
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    mm::init();
    println!("[kernel] back to world!");
    // mm::remap_test();
    trap::init();

    // Set sie.stie(Supervisor Timer Interrupt Enable) field
    // so that S privileged clock interrupts are not masked.
    trap::enable_timer_interrupt();

    // Set the first 10 ms timer.
    timer::set_next_trigger();

    // When the CPU receives an S-state clock interrupt in the U-state,
    // it is preempted and then enters the Trap process,
    // regardless of whether the sstatus.SIE bit is set or not.
    task::run_first_task();

    panic!("Unreachable in rust_main!");
}
