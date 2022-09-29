#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

#[macro_use]
extern crate bitflags;

use alloc::vec::Vec;
use buddy_system_allocator::LockedHeap;
use syscall::*;

const USER_HEAP_SIZE: usize = 32768; // 32KiB

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    // command arguments str
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        // Get the starting address of the command argument string from the 1st address of the argv array.
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        // Look for the 0 that represents the end of the command arg you put in os/task/task.rs
        // to get the end address.
        let len = (0usize..)
            // null character('\0') is an integer constant with the value zero.
            // - https://theasciicode.com.ar
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
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

bitflags! {
    pub struct OpenFlags: u32 {
        /// It is opened in read-only mode
        const RDONLY = 0;
        /// It is opened in write-only mode.
        const WRONLY = 1 << 0;
        /// Both read and write.
        const RDWR = 1 << 1;
        /// `CREATE` of the file is allowed and should be created if it is not found;
        /// if it already exists, the file size should be set to zero.
        const CREATE = 1 << 9;
        /// It should be cleared and the size set back to zero,
        /// i.e. `TRUNC`, when opening the file.
        const TRUNC = 1 << 10;
    }
}

/// Opens a regular file and returns an accessible file descriptor.
///
/// # Parameters
/// - `path`: Describe the filename of the file to be opened (for simplicity,
/// the file system does not need to support directories, all files are placed in the root(`/`) directory).
/// - `flags`: Describe the flags to be used when opening the file.
///
/// # Flags
///
/// | flags-bit |  permission  |                               Meaning                                     |
/// |-----------|--------------|---------------------------------------------------------------------------|
/// |-----------|--------------|---------------------------------------------------------------------------|
/// |         0 |    read-only | it is opened in read-only mode `RDONLY`.                                  |
/// |-----------|--------------|---------------------------------------------------------------------------|
/// |  0(0x001) |   write-only | it is opened in write-only mode `WRONLY`.                                 |
/// |-----------|--------------|---------------------------------------------------------------------------|
/// |  1(0x002) | read & write | `RDWR` for both read and write.                                           |
/// |-----------|--------------|---------------------------------------------------------------------------|
/// |  9(0x200) |       create | `CREATE` of the file is allowed and should be created if it is not found; |
/// |           |              | if it already exists, the file size should be set to zero.                |
/// |-----------|--------------|---------------------------------------------------------------------------|
/// | 10(0x400) |        trunc | it should be cleared and the size set back to zero,                       |
/// |           |              | i.e. `TRUNC`, when opening the file.                                      |
/// |-----------|--------------|---------------------------------------------------------------------------|
///
/// # Return
/// Conditional branching.
/// - if there is an error => -1
/// - otherwise=> returns the file descriptor of the file normally.
///               Possible error cause: the file does not exist.
///
/// # Example
/// ```rust
/// #[macro_use]
/// extern crate user_lib;
///
/// use user_lib::{close, open, write, OpenFlags};
///
/// let test_str = "Hello, world!";
/// let file_a = "file_a\0";
/// let fd = open(file_a, OpenFlags::CREATE | OpenFlags::WRONLY);
/// assert!(fd > 0);
/// let fd = fd as usize;
/// write(fd, test_str.as_bytes());
/// close(fd);
/// ```
pub fn open(path: &str, flags: OpenFlags) -> isize {
    sys_open(path, flags.bits)
}

/// The current process closes the file.
///
/// # Parameter
/// - `fd`: File descriptor of the file to close.
///
/// # Return
/// Conditional branching.
/// - if the process terminated successfully => 0
/// - otherwise => -1
///   - Error cause: the file descriptor passed may not correspond to the file being opened.
pub fn close(fd: usize) -> isize {
    sys_close(fd)
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

// Get current time.
pub fn get_time() -> isize {
    sys_get_time()
}

/// Get process id.
pub fn getpid() -> isize {
    sys_getpid()
}

/// Create a child process with a new address space that inherits the stack of the parent process.
/// The current process forks a child process.
///
/// # Return
/// - If child process => 0
/// - If current process => PID(Process Identifier) of child process
pub fn fork() -> isize {
    sys_fork()
}

/// Clear the address space of the current process, load a specific executable file,
/// return to the user state, and begin its execution.
///
/// # Parameter
/// - `path`: Name of the executable to load.
/// - `args`: Array of starting addresses for command line parameter strings.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1 (e.g. no executable file with matching name found)
/// - Otherwise => The length of `args` array
pub fn exec(path: &str, args: &[*const u8]) -> isize {
    sys_exec(path, args)
}

/// Wait for any child process to exit.
///
/// When a waiting child process exists but has not yet terminated,
/// call `yield_` to actively surrender CPU usage,
/// and then call sys_waitpid again when CPU usage is returned from the kernel to check
/// whether the waiting child process has terminated,
/// thereby reducing waste of CPU resources.
///
/// # Parameter
/// - `exit_code`: Address where the return value of the child process is stored.
///   If this address is 0, it means that there is no need to store the return value.
///
/// # Return
/// Conditional branching.
/// - If not already stopped => call `yield_` & return 0
/// - exit => The process ID of the terminated child process
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

/// Open a pipe for the current process.
///
/// # Parameter
/// - `pipe_fd`: Starting address of a usize array of length 2 in the application address space.
///
///   The kernel must write the file descriptors of the read and write sides of the pipe in order.
///   The write side of the file descriptor is stored in the array.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1
/// - Otherwise => a possible cause of error is that the address passed is an invalid one.
pub fn pipe(pipe_fd: &mut [usize]) -> isize {
    sys_pipe(pipe_fd)
}
