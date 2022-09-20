#![no_std]
#![no_main]

extern crate alloc;

mod bitmap;
mod block_cache;
mod block_dev;
mod layout;

/// 1 sector == 512byte
///
/// the default block size for the Linux Ext4 file system is 4096 bytes.
/// easy-fs's implementation equates blocks and sectors to 512 bytes.
pub const BLOCK_SZ: usize = 512;
