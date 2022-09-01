use core::arch::asm;

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

const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;

/// Write the data in the buffer in memory to the file.
/// - syscall ID: 64
///
/// # Parameters
/// -  `fd`: indicates the file descriptor of the file to be written.
/// - `buf`: indicates the start address of the in-memory buffer.
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
/// - `xstate`: indicates the return value of the application.
///
/// # Return
/// This system call should not return.
pub fn sys_exit(xstate: i32) -> isize {
    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
}

/// The application actively relinquishes ownership of the CPU and switches to another application.
/// - Syscall ID: 124
///
/// # Return
/// always returns 0.
pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}

pub fn sys_get_time() -> isize {
    syscall(SYSCALL_GET_TIME, [0, 0, 0])
}
