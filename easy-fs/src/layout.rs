use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter, Result};

/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;
/// The max number of direct inodes
const INODE_DIRECT_COUNT: usize = 28;
/// The max length of inode name
const NAME_LENGTH_LIMIT: usize = 27;
/// The max number of indirect1 inodes
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
/// The max number of indirect2 inodes
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
/// The upper bound of direct inode index
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
/// The upper bound of indirect1 inode index
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
/// The upper bound of indirect2 inode indexes
#[allow(unused)]
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;

/// Super block of a filesystem
///
/// `SuperBlock` is an on-disk data structure that is stored at the beginning of block number 0 on the disk.
#[repr(C)]
pub struct SuperBlock {
    /// Magic number for file system validation.
    magic: u32,
    /// Total number of blocks in the file system.
    ///
    /// # Note
    /// This is different from the total number of blocks on the disk,
    /// since the file system may not occupy the entire disk.
    pub total_blocks: u32,
    /// It is a bitmap of the index node and is as long as the number of blocks.
    ///
    /// It records the index nodes in subsequent index node areas
    /// that have been allocated for use and those that have not yet been allocated.
    pub inode_bitmap_blocks: u32,
    /// The index node area is several blocks long. Each of these blocks contains several index nodes.
    pub inode_area_blocks: u32,
    /// The data block bitmap, which is as long as a number of blocks.
    ///
    /// Subsequent block areas record which blocks have been used and which have not yet been allocated.
    pub data_bitmap_blocks: u32,
    /// In the data block area, as the name implies,
    /// each allocated block holds the specific data contents of a file or directory.
    pub data_area_blocks: u32,
}

impl Debug for SuperBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("SuperBlock")
            .field("total_blocks", &self.total_blocks)
            .field("inode_bitmap_blocks", &self.inode_bitmap_blocks)
            .field("inode_area_blocks", &self.inode_area_blocks)
            .field("data_bitmap_blocks", &self.data_bitmap_blocks)
            .field("data_area_blocks", &self.data_area_blocks)
            .finish()
    }
}

impl SuperBlock {
    /// Can be used to initialize `SuperBlock` when creating easy-fs.
    ///
    /// # Note
    /// The number of blocks in each region is passed as a parameter,
    /// and its division is the job of the higher-level disk block manager.
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }

    /// Check if a super block is valid using efs magic
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

/// Type of a disk inode
#[derive(PartialEq, Eq)]
pub enum DiskInodeType {
    File,
    Directory,
}

/// A indirect block
type IndirectBlock = [u32; BLOCK_SZ / 4];
/// A data block
type DataBlock = [u8; BLOCK_SZ];

/// A disk inode
///
/// - 128 bytes
///
/// |    name   |  type   |  size |
/// |-----------|---------|-------|
/// |    size   |  u32    |  4byte|
/// |   direct  |  u32*28 |112byte|
/// | indirect1 |  u32    |  4byte|
/// | indirect2 |  u32    |  4byte|
/// |    type_  |  i32    |  4byte|
///
/// #\[repr(C)\] enum
/// - sw: store word(32bit)
/// - can use -1 -> i32
/// - when use 1 -> use sw(i.e. Even if it is less than 255 (8 bits), 32 bits are stored in the stack.)
///
/// - https://godbolt.org/z/34f8cczq6
#[repr(C)]
pub struct DiskInode {
    /// Bytes of file/directory content
    pub size: u32,
    /// Index of the data block that stores the contents of the file/directory
    ///
    /// BLOCK_SZ(512byte) * INODE_DIRECT_COUNT(28) =14,336 = 14KiB
    ///
    /// - BLOCK_SZ(512byte): 512 * 8 = 4096 bit
    pub direct: [u32; INODE_DIRECT_COUNT],
    /// The first level index block in the data block area of the disk layout.
    ///
    /// Index for storing sizes larger than 14KiB specifiable in direct.
    ///
    /// Each u32 of this first-level index block is used to point to a data block
    /// in the data block area that holds the contents of the file, thus up to
    ///
    /// 512byte(1block) / 4 = 128byte, 128 * BLOCK_SZ(512byte) = 64KiB of content.
    ///
    ///  - Divide 4: To make efficient use of space, the DiskInode size is set to 128 bytes, so that each block can hold exactly four DiskInodes.
    pub indirect1: u32,
    /// Each u32 in the secondary index block refers to a different primary index block in the data block area.
    /// Therefore, up to
    ///
    /// 128(512byte(1block) / 4) x 64KiB(max size that can be specified by direct1 ) = 8MiB
    ///
    /// can be indexed in the secondary indirect index.
    pub indirect2: u32,
    /// File/Directory
    type_: DiskInodeType,
}

impl DiskInode {
    /// Initialize a disk inode, as well as all direct inodes under it
    /// indirect1 and indirect2 block are allocated only when they are needed
    pub fn initialize(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }

    /// Whether this inode is a directory
    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }

    /// Whether this inode is a file
    #[allow(unused)]
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }

    /// Return block number correspond to size.
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }

    /// Round up the size given as argument to multiples of Block size.
    fn _data_blocks(size: u32) -> u32 {
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }

    /// Return number of blocks needed include indirect1/2.
    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks;
        // indirect1
        if data_blocks > INODE_DIRECT_COUNT {
            total += 1;
        }
        // indirect2
        if data_blocks > INDIRECT1_BOUND {
            total += 1;
            // sub indirect1
            total +=
                (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
        }
        total as u32
    }

    /// Get the number of data blocks that have to be allocated given the new size of data
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }

    /// Get id of block given inner id
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect2: &IndirectBlock| {
                    indirect2[last / INODE_INDIRECT1_COUNT]
                });
            get_block_cache(indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect1: &IndirectBlock| {
                    indirect1[last % INODE_INDIRECT1_COUNT]
                })
        }
    }

    /// Increase the size of current disk inode
    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>,
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();
        // fill direct
        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        // alloc indirect1
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else {
            return;
        }
        // fill indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect1[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        // alloc indirect2
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        } else {
            return;
        }
        // fill indirect2 from (a0, b0) -> (a1, b1)

        // old indirect1 inode group
        let mut a0 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        // old indirect1 inode position
        let mut b0 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        // new indirect1 inode group
        let a1 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        // new indirect1 inode position
        let b1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        // alloc low-level indirect1
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                while (a0 < a1) || (a0 == a1 && b0 < b1) {
                    if b0 == 0 {
                        indirect2[a0] = new_blocks.next().unwrap();
                    }
                    // fill current
                    get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            indirect1[b0] = new_blocks.next().unwrap();
                        });
                    // move to next
                    b0 += 1;
                    // When the maximum position in the inode group is reached,
                    // the index is shifted to the next group.
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            });
    }

    /// Clear size to zero and return blocks that should be deallocated.
    /// We will clear the block contents to zero later.
    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        let mut current_blocks = 0usize;
        // direct
        while current_blocks < data_blocks.min(INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }
        // indirect1 block
        if data_blocks > INODE_DIRECT_COUNT {
            v.push(self.indirect1);
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        } else {
            return v;
        }
        // indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < data_blocks.min(INODE_INDIRECT1_COUNT) {
                    v.push(indirect1[current_blocks]);
                    //indirect1[current_blocks] = 0;
                    current_blocks += 1;
                }
            });
        self.indirect1 = 0;
        // indirect2 block
        if data_blocks > INODE_INDIRECT1_COUNT {
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            return v;
        }
        // indirect2
        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let a1 = data_blocks / INODE_INDIRECT1_COUNT;
        let b1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                // full indirect1 blocks
                for entry in indirect2.iter_mut().take(a1) {
                    v.push(*entry);
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }
                // last indirect1 block
                if b1 > 0 {
                    v.push(indirect2[a1]);
                    get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(b1) {
                                v.push(*entry);
                            }
                        });
                    //indirect2[a1] = 0;
                }
            });
        self.indirect2 = 0;
        v
    }

    /// Read data from current disk inode
    ///
    /// # parameters
    ///
    /// - `offset`: The starting point of the block to be read.
    /// - `buf`: Location to store read data.
    /// - `block_device`: The structure in which the methods of the `File` trait are implemented.
    ///                   The `read` method defined in the `FIle` trait is read inside the function.
    ///
    /// # Return
    ///  Conditional branching.
    /// - If offset is greater than `offset + buf length` or  `self.size(file/dir size)` => 0
    /// - otherwise => Length of data finished reading (`buf` same as length of copied data)
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;
        let mut read_size = 0usize;
        loop {
            // calculate end of current block
            //
            // # Example
            // 5000 / 512 = 9
            // 9 + 1 =10
            // 10 * 512 = 5120
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            // read and update read size
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |data_block: &DataBlock| {
                // data_block is 1Block.
                // `start % BLOCK_SZ` index of 1Block
                let src = &data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size];
                dst.copy_from_slice(src);
            });
            read_size += block_read_size;
            // move to next block
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        read_size
    }

    /// Write data into current disk inode
    /// size must be adjusted properly beforehand
    ///
    /// # parameters
    /// - `offset`: The starting point of the block to be read.
    /// - `buf`: Data to be written.
    /// - `block_device`: The structure in which the methods of the `File` trait are implemented.
    ///                   The `read` method defined in the `FIle` trait is read inside the function.
    /// # Panic
    /// 1st argument `offset` is greater than `offset + buf length` or  `self.size(file/dir size)`
    ///
    /// # Return
    /// Length of data that has been written
    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        assert!(start <= end);
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop {
            // calculate end of current block
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            // write and update write size
            let block_write_size = end_current_block - start;
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                let src = &buf[write_size..write_size + block_write_size];
                let dst = &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                dst.copy_from_slice(src);
            });
            write_size += block_write_size;
            // move to next block
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
    }
}

/// A directory entry
#[repr(C)]
pub struct DirEntry {
    /// # Why +1?
    /// +1: "\0"
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_number: u32,
}

/// Size of a directory entry
pub const DIRENT_SZ: usize = 32;

impl DirEntry {
    /// Create an empty directory entry
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }

    /// Crate a directory entry from name and inode number
    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8; NAME_LENGTH_LIMIT + 1];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }

    /// Serialize into bytes
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, DIRENT_SZ) }
    }

    /// Serialize into mutable bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, DIRENT_SZ) }
    }

    /// Get name of the entry
    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }

    /// Get inode number of the entry
    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}
