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
    exit(main(argc, v.as_slice()));
}

// Use the main logic of the application in the bin directory as the main logic
// even if there are main symbols in both the lib.rs and bin directories
#[linkage = "weak"]
#[no_mangle]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
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

/// Duplicates the file descriptor reference passed in the argument.
///
/// # Parameter
/// - `fd`: The file descriptor of a file already open in the process.
///
/// # Return
/// Conditional branching.
/// - if an error occurred => -1,
/// - otherwise => the new file descriptor of the opened file is accessible.
/// A possible cause of the error is that the passed fd does not correspond to a legal open file.
pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
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
/// - If none of the waiting child processes have exited => execute `yield_` & loop
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

/// Action for a signal
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    // address of signal handle routine
    pub handler: usize,
    // signal mask
    pub mask: SignalFlags,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            handler: 0,
            mask: SignalFlags::empty(),
        }
    }
}

bitflags! {
    /// Signals
    /// - https://www.gnu.org/software/libc/manual/html_node/Job-Control-Signals.html
    pub struct SignalFlags: i32 {
        /// Default behavior: kill process
        const SIGDEF = 1;
        /// Hang-up, termination of controlled terminal.
        const SIGHUP = 1 << 1;
        /// signal interrupt
        /// - sent when CTRL+C is pressed in the current process.
        const SIGINT    = 1 << 2;
        const SIGQUIT = 1 << 3;
        /// Exceptions to False Orders
        const SIGILL    = 1 << 4;
        const SIGTRAP = 1 << 5;
        /// signal abort
        /// - Generated by a call to the abort function,
        ///   causing the process to terminate abnormally.
        const SIGABRT   = 1 << 6;
        const SIGBUS = 1 << 7;
        const SIGFPE    = 1 << 8;
        /// Force the process to terminate
        const SIGKILL = 1 << 9;
        /// User defined signal 1
        const SIGUSR1 = 1 << 10;
        /// signal segmentation violation
        /// - Illegal memory access exception
        const SIGSEGV = 1 << 11;
        /// User defined signal 2
        const SIGUSR2 = 1 << 12;
        const SIGPIPE = 1 << 13;
        const SIGALRM = 1 << 14;
        const SIGTERM = 1 << 15;
        const SIGSTKFLT = 1 << 16;
        /// signal child
        /// - Sent to a parent process whenever one of its child processes terminates or stops.
        const SIGCHLD = 1 << 17;
        /// signal continue
        /// - Signal to cancel pause
        const SIGCONT = 1 << 18;
        /// signal stop
        /// - Suspends the process
        const SIGSTOP = 1 << 19;
        /// `CTRL+Z` key pressed in current process will be sent to current process to pause
        const SIGTSTP = 1 << 20;
        const SIGTTIN = 1 << 21;
        const SIGTTOU = 1 << 22;
        const SIGURG = 1 << 23;
        const SIGXCPU = 1 << 24;
        const SIGXFSZ = 1 << 25;
        const SIGVTALRM = 1 << 26;
        const SIGPROF = 1 << 27;
        const SIGWINCH = 1 << 28;
        const SIGIO = 1 << 29;
        const SIGPWR = 1 << 30;
        const SIGSYS = 1 << 31;
    }
}

impl SignalFlags {
    /// Get bit digit
    ///
    /// # Example
    /// ```rust
    /// let user_digit = SignalFlags::to_bit_digit(SignalFlags::SIGUSR1) as i32;
    /// assert_eq!(user_digit, 10);
    /// ```
    pub fn to_bit_digit(bits: SignalFlags) -> u32 {
        SignalFlags::log_2(bits.bits())
    }

    /// # Example
    ///
    /// 1 << 19 = 0000 0000 0000 1000 0000 0000 0000 0000
    ///
    /// leading_zeros => 12
    ///
    /// 32 - 12 - 1 = 19
    fn log_2(x: i32) -> u32 {
        (core::mem::size_of::<i32>() * 8) as u32 - x.leading_zeros() - 1
    }
}

/// Send a signal to the process
///
/// # Parameters
/// - `pid`: ID of the process
/// - `signal`: integer value representing the signal
///
/// # Return
/// Conditional branching.
/// - If the bit corresponding to `signum` in the signal of the process control block is successfully
///   set to 1. => 0
///
/// - No `TaskControlBlock` corresponding to `pid`(1st arg) => -1
/// - no `signal` corresponding to `signum` => -1
/// - If the bit of `signum` is already included in `signals` in the `TaskControlBlockInner`
///   corresponding to `pid` => -1
///
/// # Information
/// It is to send a signal with the value signum to the process with process number pid.
/// Specifically, it finds the process control block by `pid` and sets the bit corresponding to `signum`
/// in the signal of that process control block to 1.
pub fn kill(pid: usize, signal: i32) -> isize {
    sys_kill(pid, signal)
}

/// Registers a new handler (`action` argument) corresponding to the `signum` given as argument
/// and writes the original handler to `old_action`.
///
/// # Parameters
/// - `signum`: A signal bit digit corresponding to the process to be registered.
/// - `action`: new signal processing configuration
/// - `old_action`: old signal processing configuration
///
/// # Return
/// Conditional branching.
/// - If the `signum` and `action` arguments are successfully tied together => 0
///
/// - Failed to get the current task context => -1
/// - If `signum` exceeds the bit digits of `MAX_SIG` => -1
/// - If `action` or `old_action` is 0, or `signum` is `SIGKILL(1<<9)` or `SIGSTOP(1<<19)` => -1
pub fn sigaction(
    signum: i32,
    action: *const SignalAction,
    old_action: *const SignalAction,
) -> isize {
    sys_sigaction(signum, action, old_action)
}

/// Set signal to block
///
/// # Parameters
/// - `mask`: signal mask
///
/// # Panic
/// When casting mask to SignalFlags fails.
///
/// # Return
/// Conditional branching.
/// - When `signal_mask` is rewritten successfully => old `signal_mask`
/// - Otherwise => -1
pub fn sigprocmask(mask: u32) -> isize {
    sys_sigprocmask(mask)
}

/// Set the signal being processed to -1 (none) and restoring a backup of a trap context.
///
///  # Information
/// A recovery operation performed by the signal handler after it has finished responding to a signal,
/// i.e., restoring the trap context saved by the operating system before responding to the signal,
/// so that execution can continue from where the process was normally running before the signal was
/// processed.
///
/// # Return
/// Conditional branching.
/// - Success to restore a backup of a trap context => 0
/// - Otherwise => -1
pub fn sigreturn() -> isize {
    sys_sigreturn()
}

/// Current process creates a new thread.
///
/// # Parameters
/// - `entry`: The address of the entry function of the thread.
/// - `arg`: The argument to the thread.
///
/// # Return
/// new thread ID
pub fn thread_create(entry: usize, arg: usize) -> isize {
    sys_thread_create(entry, arg)
}

/// Keep yielding and wait until the Thread with the ID specified in the argument closes.
///
/// # Parameter
/// - `tid`: thread id
///
/// # Return
/// Conditional branching.
/// - If the thread does not exist => -1
/// - If the thread has not yet exited(-2) => call `yield` and wait
/// - In other cases => The exit code of the ending thread
pub fn waittid(tid: usize) -> isize {
    loop {
        match sys_waittid(tid) {
            -2 => {
                yield_();
            }
            exit_code => return exit_code,
        }
    }
}

/// Create a new exclusion control.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Return
/// Index of the lock list within one process of the created Mutex.
pub fn mutex_create() -> isize {
    sys_mutex_create(false)
}

/// Create a new exclusion blocking control.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Return
/// Index of the lock list within one process of the created Mutex.
pub fn mutex_blocking_create() -> isize {
    sys_mutex_create(true)
}

/// **Lock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **lock**
///
/// # Return
/// always 0
pub fn mutex_lock(mutex_id: usize) {
    sys_mutex_lock(mutex_id);
}

/// **Unlock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **unlock**
///
/// # Return
/// always 0
pub fn mutex_unlock(mutex_id: usize) {
    sys_mutex_unlock(mutex_id);
}

/// Create a new exclusion control.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Parameter
/// - `res_count`:Number of threads with concurrent access to shared resources.
///
/// ## Counting(General) semaphores (`res_count` >= 2):
/// - Allow multiple threads with a maximum of `res_count` to access critical sections simultaneously
///
/// ## Binary semaphores(`res_count` == 1):
/// - Only one thread has access to the critical section.
/// - Semaphores restricted to values 0 and 1 (or locked/unlocked, disabled/enabled).
/// - Provide similar functionality to `Mutex`.
///
/// # Return
/// Index of the lock list within one process of the created `Semaphore`.
pub fn semaphore_create(res_count: usize) -> isize {
    sys_semaphore_create(res_count)
}

/// # V (Verhogen (Dutch), increase) operation
/// Increment semaphores(`self.count`)
///
/// If `self.count` is less than or equal to 0, a waiting thread is popped
/// from the top of the queue and added to the task queue (for the task to be executed).
///
/// # Return
/// always 0
pub fn semaphore_up(sem_id: usize) {
    sys_semaphore_up(sem_id);
}

/// # P (Proberen (Dutch), try) operation
/// Decrement semaphores(`self.count`)
///
/// If `self.count` is less than 0, the currently running thread is added to the
/// end of `self.wait_queue` and continues waiting for the lock to be released in the `Blocking` state.
///
/// # Return
/// always 0
pub fn semaphore_down(sem_id: usize) {
    sys_semaphore_down(sem_id);
}
