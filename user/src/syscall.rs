use crate::SignalAction;
use core::arch::asm;

const SYSCALL_DUP: usize = 24;
const SYSCALL_OPEN: usize = 56;
const SYSCALL_CLOSE: usize = 57;
const SYSCALL_PIPE: usize = 59;
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_KILL: usize = 129;
const SYSCALL_SIGACTION: usize = 134;
const SYSCALL_SIGPROCMASK: usize = 135;
const SYSCALL_SIGRETURN: usize = 139;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;
const SYSCALL_THREAD_CREATE: usize = 1000;
const SYSCALL_WAITTID: usize = 1002;
const SYSCALL_MUTEX_CREATE: usize = 1010;
const SYSCALL_MUTEX_LOCK: usize = 1011;
const SYSCALL_MUTEX_UNLOCK: usize = 1012;

#[inline(always)]
fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        // x10: a0, x11: a1, x12: a2 -> x10: system call result
        // ecall(x10)
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}

/// Duplicates the file descriptor reference passed in the argument.
/// - syscall ID: 24
///
/// # Parameter
/// - `fd`: The file descriptor of a file already open in the process.
///
/// # Return
/// Conditional branching.
/// - if an error occurred => -1,
/// - otherwise => the new file descriptor of the opened file is accessible.
/// A possible cause of the error is that the passed fd does not correspond to a legal open file.
pub fn sys_dup(fd: usize) -> isize {
    syscall(SYSCALL_DUP, [fd, 0, 0])
}

/// Opens a regular file and returns an accessible file descriptor.
/// - syscall ID: 56
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
pub fn sys_open(path: &str, flags: u32) -> isize {
    syscall(SYSCALL_OPEN, [path.as_ptr() as usize, flags as usize, 0])
}

/// The current process closes the file.
/// - syscall ID: 57
///
/// # Parameter
/// - `fd`: File descriptor of the file to close.
///
/// # Return
/// Conditional branching.
/// - if the process terminated successfully => 0
/// - otherwise => -1
///   - Error cause: the file descriptor passed may not correspond to the file being opened.
pub fn sys_close(fd: usize) -> isize {
    syscall(SYSCALL_CLOSE, [fd, 0, 0])
}

/// Open a pipe for the current process.
/// - syscall ID: 59
///
/// # Parameter
/// - `pipe`: Starting address of a usize array of length 2 in the application address space.
///
///   The kernel must write the file descriptors of the read and write sides of the pipe in order.
///   The write side of the file descriptor is stored in the array.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1
/// - Otherwise => a possible cause of error is that the address passed is an invalid one.
pub fn sys_pipe(pipe: &mut [usize]) -> isize {
    syscall(SYSCALL_PIPE, [pipe.as_mut_ptr() as usize, 0, 0])
}

/// Reads a piece of content from a file into a buffer.
/// - syscall ID: 63
///
/// # parameters
/// - `fd`: File descriptor of the file to read.
/// - `buffer`: The start address of the in-memory buffer.
///
/// # Return
/// Conditional branching.
/// - If an error occurs => -1
/// - otherwise => number of bytes actually read.
pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(
        SYSCALL_READ,
        [fd, buffer.as_mut_ptr() as usize, buffer.len()],
    )
}

/// Write the data in the buffer in memory to the file.
/// - syscall ID: 64
///
/// # Parameters
/// -  `fd`: The file descriptor of the file to be written.
/// - `buffer`: indicates the start address of the in-memory buffer.
///
/// # Return
/// The length of the successful write.
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

/// Exit the application and inform the batch system of the return value.
/// - syscall ID: 93
///
/// # Parameters
/// - `xstate`: The return value of the application.
///
/// # Panic
/// If there is a return value.
pub fn sys_exit(xstate: i32) -> ! {
    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0]);
    panic!("sys_exit never returns!");
}

/// The application actively relinquishes ownership of the CPU and switches to another application.
/// - syscall ID: 124
///
/// # Return
/// always returns 0.
pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}

/// Send a signal to the process
/// - syscall ID: 129
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
pub fn sys_kill(pid: usize, signal: i32) -> isize {
    syscall(SYSCALL_KILL, [pid, signal as usize, 0])
}

// Get current time.
/// - syscall ID: 169
pub fn sys_get_time() -> isize {
    syscall(SYSCALL_GET_TIME, [0, 0, 0])
}

/// Get process id.
/// - syscall ID: 172
pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0])
}

/// Create a child process with a new address space that inherits the stack of the parent process.
/// The current process forks a child process.
/// - syscall ID: 220
///
/// # Return
/// - If child process => 0
/// - If current process => PID(Process Identifier) of child process
///
/// # Details
///
/// After process A calls the fork system call, the kernel creates a new process B.
/// This process B and the process A that invoked fork are in approximately the same state
/// at the time each returns to the user state.
/// This means that they both have the exact same user state code, stack, and other data segments,
/// but they are located in two separate address spaces. Therefore,
/// the address space of the new process must be copied completely from the address space of the original process.
///
/// Also, the general-purpose registers are almost identical in both processes.
/// For example, the fact that `pc` (program counter) is identical means that both processes start
/// from the same instruction in the same location (knowing that the previous instruction must be
/// an `ecall` instruction for a system call), and the fact that sp (stack pointer) is identical means
/// that the user stack of both processes is identical. The fact that sp(stack pointer) is identical means
/// that the user stacks of both processes are in the same place in their respective address spaces.
/// The remaining registers are identical to ensure that they return to the same control flow state.
///
/// However, only the value of the `a0` register that stores the return value of the fork system call
/// (this is the register used for the return value of the function specified in the RISC-V 64 function call
/// specification) is different. This distinguishes the two processes,
/// as the return value of the original process is the PID of its newly created process,
/// whereas the return value of the newly created process is 0.
/// Since the new process is derived from the fork that the original process actively invoked,
/// the original process is called the parent process of the new process, whereas the new process
/// is called the child process of the original process. This creates a parent-child relationship between them.
/// Noting that each process can have multiple child processes but at most one parent process,
/// all processes can be organized into a tree, the root of which represents initproc,
/// the first initial process  in user state.
pub fn sys_fork() -> isize {
    syscall(SYSCALL_FORK, [0, 0, 0])
}

/// Clear the address space of the current process, load a specific executable file,
/// return to the user state, and begin its execution.
/// - syscall ID: 221
///
/// # Parameter
/// - `path`: Name of the executable to load.
/// - `args`: Array of starting addresses for command line parameter strings.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1 (e.g. no executable file with matching name found)
/// - Otherwise => The length of `args` array
pub fn sys_exec(path: &str, args: &[*const u8]) -> isize {
    // Since path as type `&str` is a fat pointer that contains both the starting address and length information,
    // only the starting address is passed to the kernel using `as_ptr()` when making system calls.
    syscall(
        SYSCALL_EXEC,
        [path.as_ptr() as usize, args.as_ptr() as usize, 0],
    )
}

/// The current process waits for a child process to become a zombie process, collects all resources,
/// and collects its return value.
/// - syscall ID: 260
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
pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    syscall(SYSCALL_WAITPID, [pid as usize, exit_code as usize, 0])
}

/// Registers a new handler (`action` argument) corresponding to the `signum` given as argument
/// and writes the original handler to `old_action`.
/// - syscall ID: 134
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
pub fn sys_sigaction(
    signum: i32,
    action: *const SignalAction,
    old_action: *const SignalAction,
) -> isize {
    syscall(
        SYSCALL_SIGACTION,
        [signum as usize, action as usize, old_action as usize],
    )
}

/// Set signal to block
/// - syscall ID: 135
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
pub fn sys_sigprocmask(mask: u32) -> isize {
    syscall(SYSCALL_SIGPROCMASK, [mask as usize, 0, 0])
}

/// Set the signal being processed to -1 (none) and restoring a backup of a trap context.
/// - syscall ID: 139
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
pub fn sys_sigreturn() -> isize {
    syscall(SYSCALL_SIGRETURN, [0, 0, 0])
}

/// Current process creates a new thread.
/// - syscall ID: 139
///
/// # Parameters
/// - `entry`: The address of the entry function of the thread.
/// - `arg`: The argument to the thread.
///
/// # Return
/// new thread ID
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    syscall(SYSCALL_THREAD_CREATE, [entry, arg, 0])
}

/// Gets the status of whether the Thread with the specified ID is waiting or not.
///
/// If it is waiting, deletes the thread with the ID from the array of waiting threads and returns an exit code.
/// - syscall ID: 139
///
/// # Parameter:
/// - `tid`: thread id
///
/// # Return
/// Conditional branching.
/// - If the thread does not exist => -1
/// - If the thread has not yet exited => -2
/// - In other cases => The exit code of the ending thread
///
/// # Determining whether or not a thread is waiting
/// 1. Is there a thread with the same Theard ID?
/// 2. Is there a thread with that ID in the waiting thread array?
/// 3. is the exit_code already stored in the internal thread information?
pub fn sys_waittid(tid: usize) -> isize {
    syscall(SYSCALL_WAITTID, [tid, 0, 0])
}

/// Create a new exclusion control.
/// - syscall ID: 1010
///
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Parameter
/// - `blocking`: use `MutexBlocking`?
///
/// # Return
/// Index of the lock list within one process of the created Mutex.
pub fn sys_mutex_create(blocking: bool) -> isize {
    syscall(SYSCALL_MUTEX_CREATE, [blocking as usize, 0, 0])
}

/// **Lock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
/// - syscall ID: 1011
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **lock**
///
/// # Return
/// always 0
pub fn sys_mutex_lock(id: usize) -> isize {
    syscall(SYSCALL_MUTEX_LOCK, [id, 0, 0])
}

/// **Unlock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
/// - syscall ID: 1012
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **unlock**
///
/// # Return
/// always 0
pub fn sys_mutex_unlock(id: usize) -> isize {
    syscall(SYSCALL_MUTEX_UNLOCK, [id, 0, 0])
}
