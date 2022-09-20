use core::any::Any;
use core::marker::{Send, Sync};

pub trait BlockDevice: Send + Sync + Any {
    ///  Reads the block number `block_id` from disk to the buffer `buf` in memory.
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    /// Writes the data in memory buffer `buf` to the block numbered by `block_id` on disk.
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
