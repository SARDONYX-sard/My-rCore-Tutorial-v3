#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

use buddy_system_allocator::LockedHeap;
use syscall::*;

const USER_HEAP_SIZE: usize = 16384; // 16KiB

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    exit(main());
}

// Use the main logic of the application in the bin directory as the main logic
// even if there are main symbols in both the lib.rs and bin directories
#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!")
}

/// Reads a piece of content from a file into a buffer.
///
/// # Parameters
/// - `fd`: File descriptor of the file to read.
/// - `buf`: The start address of the in-memory buffer.
///
/// # Return
/// Conditional branching.
/// - If an error occurs => -1
/// - otherwise => number of bytes actually read.
///
/// # Examples
/// ```
/// const STDIN: usize = 0;
/// const s: &str = "Hello"
/// let result = read(STDIN, s.as_bytes());
/// assert_eq!(result, "Hello");
/// ```
pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}

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
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

/// The application actively relinquishes ownership of the CPU and switches to another application.
///
/// # Return
/// always returns 0.
pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

/// Clear the address space of the current process, load a specific executable file,
/// return to the user state, and begin its execution.
///
/// # Parameter
/// - `path`: Name of the executable to load.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1 (e.g. no executable file with matching name found)
/// - Otherwise => do not return.
pub fn exec(path: &str) -> isize {
    sys_exec(path)
}

/// Wait for any child process to exit.
///
/// When a waiting child process exists but has not yet terminated,
/// call `yield_` to actively surrender CPU usage,
/// and then call sys_waitpid again when CPU usage is returned from the kernel to check
/// whether the waiting child process has terminated,
/// thereby reducing waste of CPU resources.
///
/// # Parameters
/// - `exit_code`: Address where the return value of the child process is stored.
///   If this address is 0, it means that there is no need to store the return value.
///
/// # Return
/// Conditional branching.
/// - If there is no child process to wait => -1
/// - If none of the waiting child processes have exited => -2
/// - Otherwise => The process ID of the terminated child process
pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => {
                // -2: Waiting child process exists but has not yet terminated.
                // call `yield_` to aggressively surrender CPU usage and reduce waste of CPU resources.
                yield_();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

/// The current process waits for a child process to become a zombie process, collects all resources,
/// and collects its return value.
///
/// # Parameters
/// - `pid`: Process ID of the child process to wait. If -1, it means to wait for any child process.
/// - `exit_code`: Address where the return value of the child process is stored.
///              If this address is 0, it means that there is no need to store the return value.
///
/// # Return
/// Conditional branching.
/// - If there is no child process to wait => -1
/// - If none of the waiting child processes have exited => -2
/// - Otherwise => The process ID of the terminated child process
pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => {
                yield_();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn sleep(period_ms: usize) {
    let start = sys_get_time();
    while sys_get_time() < start + period_ms as isize {
        sys_yield();
    }
}
