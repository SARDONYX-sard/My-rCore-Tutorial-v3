use core::arch::asm;

const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;

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
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1 (e.g. no executable file with matching name found)
/// - Otherwise => do not return.
pub fn sys_exec(path: &str) -> isize {
    // Since path as type `&str` is a fat pointer that contains both the starting address and length information,
    // only the starting address is passed to the kernel using `as_ptr()` when making system calls.
    syscall(SYSCALL_EXEC, [path.as_ptr() as usize, 0, 0])
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
