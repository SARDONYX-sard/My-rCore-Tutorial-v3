//! File and filesystem-related syscalls

use crate::fs::{make_pipe, open_file, OpenFlags};
use crate::mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};

/// Write the data in the buffer in memory to the file.
///
/// # Parameters
/// -  `fd`: The file descriptor of the file to be written.
/// - `buffer`: indicates the start address of the in-memory buffer.
/// - `len`: Length to write.
///
/// # Return
/// Conditional branching.
/// - If an error occurs
///   (e.g. If you put a file descriptor number in `fd` that does not exist in the file descriptor table) => -1
/// - otherwise => The length of the successful write.
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        // release current task TCB(TaskControlBlock) manually to avoid multi-borrow
        drop(inner);
        // Convert the buffer pointed to by the application's virtual address
        // into a vector of byte array slices pointed to by the kernel's virtual address.
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

/// Reads a piece of content from a file into a buffer.
///
/// # parameters
/// - `fd`: File descriptor of the file to read.
/// - `buffer`: The start address of the in-memory buffer.
/// - `len`: Length to read.
///
/// # Return
/// Conditional branching.
/// - If an error occurs
///   (e.g. If you put a file descriptor number in `fd` that does not exist in the file descriptor table) => -1
/// - otherwise => number of bytes actually read.
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        // release current task TCB(TaskControlBlock) manually to avoid multi-borrow
        drop(inner);
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
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

/// Open a pipe for the current process.
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
pub fn sys_pipe(pipe: *mut usize) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let mut inner = task.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}
