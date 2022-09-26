//! File system in os
mod inode;
mod stdio;

use crate::mm::UserBuffer;

/// File trait
pub trait File: Send + Sync {
    /// If readable
    fn readable(&self) -> bool;
    /// If writable
    fn writable(&self) -> bool;
    /// Read file to `UserBuffer`
    ///
    /// # Return
    /// Size of buffer read
    fn read(&self, buf: UserBuffer) -> usize;
    /// Write `UserBuffer` to file
    ///
    /// # Return
    /// Size of written buffer
    fn write(&self, buf: UserBuffer) -> usize;
}

pub use inode::{list_apps, open_file, OSInode, OpenFlags};
pub use stdio::{Stdin, Stdout};
