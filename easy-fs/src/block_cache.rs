use crate::block_dev::BlockDevice;
use crate::BLOCK_SZ;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;

/// `block_id`, a `BlockDevice` that implements read/write to `self.cache`, and ties the cache together.
///
/// When the BlockCache life cycle is complete and the buffer has been recycled from memory,
/// - `modified` flag is true => Write back cache memory data to disk automatically when no longer referenced.
pub struct BlockCache {
    /// cached block data
    cache: [u8; BLOCK_SZ],
    /// underlying block id
    block_id: usize,
    /// underlying block device
    block_device: Arc<dyn BlockDevice>,
    /// whether the block is dirty(the block has been modified since it was read from disk to the memory cache.)
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = [0u8; BLOCK_SZ];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }

    /// Get the address of an offset inside the cached block data
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }

    /// Gets an immutable reference to an on-disk data structure of type `T` in the buffer at the offset.
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    /// Takes a modifiable reference to an on-disk data structure.
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        // Use size_of::<T>() to find the size of type T at compile time
        // to ensure that the data structure is contained in the entire disk block and its buffer.
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    /// Synchronize changes by writing from temporary memory to permanent storage.
    ///
    /// Set `self.modified` to false
    ///
    /// This function is called automatically
    /// when `BlockCache` is no longer referenced from anywhere (i.e., at `Drop`).
    ///
    /// In fact, sync is not only called when a drop occurs; in Linux,
    /// there is usually a background process that periodically writes the contents of buffers in memory
    /// back to disk. There is also a sys_fsync system call that allows applications to proactively
    /// notify the kernel of changes to files that are synchronized to disk.
    /// This implementation is simple, so synchronization is only called when the BlockCache is deleted.
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

/// Use a block cache of 16 blocks
///
/// To avoid wasting a large amount of memory for block caching,
/// only a limited number of disk block buffers are resident in memory at the same time.
const BLOCK_CACHE_SIZE: usize = 16;

pub struct BlockCacheManager {
    /// It manages block numbers and block cache binaries. The block number
    /// is of type `usize` and the block cache is of type `Arc<Mutex<BlockCache>>`.
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    /// Initialize new `BlockCacheManager`
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            // substitute
            // 1st, check to see if the max number of cashable items has been reached.
            if self.queue.len() == BLOCK_CACHE_SIZE {
                // from front to tail
                // 2nd, delete caches that are not referenced outside the manager.
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    // Is the block cache still in use?
                    // Its strong reference count ≥ 2, i.e., determined by the existence of one copy held
                    // by the block cache manager plus several copies in use outside the block cache.
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("Run out of BlockCache!");
                }
            }
            // 3rd, load block into mem and push back
            let block_cache = Arc::new(Mutex::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }
}

lazy_static! {
    /// The global block cache manager
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}

/// Get the block cache corresponding to the given block id and block device
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}

/// Sync all block cache to block device
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}
