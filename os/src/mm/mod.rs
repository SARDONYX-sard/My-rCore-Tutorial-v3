//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::StepByOne;
use address::VPNRange;
pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
pub use frame_allocator::{frame_alloc, frame_dealloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{kernel_token, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, translated_refmut, translated_str, PageTableEntry};
pub use page_table::{PTEFlags, PageTable, UserBuffer};

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    // Initializes the physical page frame manager (including the heap data structure Vec<T>)
    // so that available physical page frames can be allocated and reclaimed.
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
