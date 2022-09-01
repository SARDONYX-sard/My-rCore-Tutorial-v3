#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    clear_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

// Use the main logic of the application in the bin directory as the main logic
// even if there are main symbols in both the lib.rs and bin directories
#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!")
}

fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }
    (start_bss as usize..end_bss as usize).for_each(|addr| unsafe {
        (addr as *mut u8).write_volatile(0);
    });
}

use syscall::*;

/// Write the data in the buffer in memory to the file.
///
/// # Parameters
/// -  `fd`: indicates the file descriptor of the file to be written.
/// - `buf`: indicates the start address of the in-memory buffer.
///
/// # Return
/// The length of the successful write.
///
/// # Examples
/// ```
/// const STDOUT: usize = 1;
/// const s: &str = "Hello"
/// write(STDOUT, s.as_bytes());
/// ```
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

/// Exit the application and inform the batch system of the return value.
///
/// # Parameters
/// - `xstate`: indicates the return value of the application.
///
/// # Return
/// This system call should not return.
pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}
