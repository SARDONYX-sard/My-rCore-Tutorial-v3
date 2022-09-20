use alloc::sync::Arc;

use crate::{block_cache::get_block_cache, block_dev::BlockDevice, BLOCK_SZ};

/// A bitmap block
/// u64 * 64 = 4096bit
type BitmapBlock = [u64; 64];

/// There are two types of bitmaps in the easy-fs layout, one for index nodes and one for data blocks.
/// Each bitmap consists of blocks of 512 bytes, or 4096 bits each,
/// and each bit represents the allocation status of the index node/block,
/// with 0 meaning unallocated and 1 meaning allocated.
///
/// Bitmap responsible for the following.
///
/// - Bit-based allocation (find bits that are 0's and set them to 1).
/// - Index node/block allocation by recycling (clearing bits to 0).
pub struct Bitmap {
    start_block_id: usize,
    /// Length per block
    blocks: usize,
}

/// It is decomposed into the following three parts to accurately identify the bits to be played back.
///
/// - block_pos: Block number in the area with bit number bit
/// - bits64_pos: the group number within the block
/// - inner_pos: group number within a group
///
/// Then allow it to be cleared.
///
/// # Return
/// (block_pos, bits64_pos, inner_pos)
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit = bit % BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

impl Bitmap {
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }
}

const BLOCK_BITS: usize = BLOCK_SZ * 8;

impl Bitmap {
    /// # Return
    /// Conditional branching.
    /// - The position of the allocated bits, corresponding to the index node/block number
    /// - If all bits have already been assigned => `None`
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        // It enumerates each block (block_id number) in the area that needs to be read or written,
        // looks for a free bit in the block, and sets it to 1.
        for block_id in 0..self.blocks {
            let pos = get_block_cache(
                block_id + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            // Consecutive data (whose length depends on the specific type) are parsed
            // into BitmapBlocks starting at buffer offset 0, and their data structures are modified.
            //
            // Since there is only one BitmapBlock in the entire block and it is exactly 512 bytes in size,
            // offset 0 is passed. Therefore, to access the entire BitmapBlock,
            // must start at the beginning of the block.
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                // The function searches for a free bit in the bitmap_block and returns its position,
                // or None if it does not exist.

                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                {
                    // modify cache
                    bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                    Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                } else {
                    None
                }
            });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }

    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }
}
