//! File and filesystem-related syscalls

use crate::fs::{open_file, OpenFlags};
use crate::mm::{translated_byte_buffer, translated_str};
use crate::sbi::console_getchar;
use crate::task::{current_task, current_user_token, suspend_current_and_run_next};

const FD_STDIN: usize = 0;
const FD_STDOUT: usize = 1;

/// write `buf` of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    // TODO: Check security of incoming arguments

    match fd {
        FD_STDOUT => {
            // Convert the buffer pointed to by the application's virtual address
            // into a vector of byte array slices pointed to by the kernel's virtual address.
            let buffers = translated_byte_buffer(current_user_token(), buf, len);
            for buffer in buffers {
                print!("{}", core::str::from_utf8(buffer).unwrap());
            }
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}

/// read `buf` of length `len`  to a file with `fd`
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            assert_eq!(len, 1, "Only support len = 1 in sys_read!");
            let mut c: usize;
            loop {
                c = console_getchar();
                if c == 0 {
                    suspend_current_and_run_next();
                    continue;
                } else {
                    break;
                }
            }
            let ch = c as u8;
            let mut buffers = translated_byte_buffer(current_user_token(), buf, len);
            unsafe {
                buffers[0].as_mut_ptr().write_volatile(ch);
            }
            1
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
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
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
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
pub fn sys_close(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    // Simply change the entry in the process control block corresponding to the file descriptor table
    // to None to indicate that it is free, which also destroys the internal reference counter type Arc,
    // which reduces the reference count of the file, and automatically regenerates the resource occupied
    // by the file when the reference count reaches zero.
    inner.fd_table[fd].take();
    0
}
