//! `Arc<Inode>` -> `OSInodeInner`: In order to open files concurrently
//! we need to wrap `Inode` into `Arc`,but `Mutex` in `Inode` prevents
//! file systems from being accessed simultaneously
//!
//! `UPSafeCell<OSInodeInner>` -> `OSInode`: for static `ROOT_INODE`,we
//! need to wrap `OSInodeInner` into `UPSafeCell`
use super::File;
use crate::{drivers::BLOCK_DEVICE, sync::UPSafeCell};
use alloc::sync::Arc;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;

pub struct OSInode {
    /// Whether the file is allowed to be read by `sys_read` or not.
    readable: bool,
    /// Whether the file is allowed to be write by `sys_write` or not.
    writable: bool,

    inner: UPSafeCell<OSInodeInner>,
}

pub struct OSInodeInner {
    ///
    ///
    /// The offset is maintained during `sys_read/write`.
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// Receive a list of files from `ROOT_INODE` and output them in order to standard output.
pub fn list_apps() {
    let apps = ROOT_INODE.ls();
    println!("/**** APPS *****");
    for app in apps.iter() {
        println!("{}", app);
    }
    println!("***************/");
}

bitflags! {
    /// FIle open mode
    pub struct OpenFlags: u32 {
        ///  read-only mode
        const RDONLY = 0;
        ///  write-only mode
        const WRONLY = 1 << 0;
        /// read & write
        const RDWR = 1 << 1;
        /// Create
        const CREATE  = 1 <<9;
        /// clear and the size set back to zero
        const TRUNC = 1 <<10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// # Return
    /// (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// When it is desired to create a file with the same name as an existing file,
/// the contents of the file are cleared.
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, mut buf: crate::mm::UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            assert_eq!(read_size, slice.len());
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }

    fn write(&self, buf: crate::mm::UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}
