//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::*;

/// manage a frame which has the same lifecycle as the tracker
///
/// Track ppn and drop(frame_dealloc) the target Page when it is no longer needed.
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Zero-fills one page (4096 bytes) of the physical page number passed as an argument.
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("frameTracker:PPN={:#x}", self.ppn.0))
    }
}

// Implement Drop Trait to achieve RAII(Resource Acquisition Is Initialization) by taking advantage
// of the fact that when a `FrameTracker` instance is retrieved,
// its drop method is automatically called by the compiler.
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// physical page number interval \[ current , end \] has never been allocated
pub struct StackFrameAllocator {
    /// First address of `PhysPageNum` which is not allocated.
    current: usize,
    /// End address of `PhysPageNum` which is not allocated.
    end: usize,
    /// Holds the physical page numbers recycled on a last-in, first-out basis.
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    /// Assign each field value of the StackFrameAllocator structure.
    /// - self.current = l.0;
    /// - self.end = r.0;
    ///
    /// # Parameters
    ///
    /// - l: left hand of Physical Page Number
    /// - r: right hand of Physical Page Number
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    /// Allocated physical memory.
    /// - Allocation Successful => Some()
    /// - Allocation failed => None()
    ///   - Exhausted memory.
    ///
    /// # Information
    ///
    /// Internally, it branches into two ways of allocation.
    ///
    /// - Pop a page from the vector of recycled memory managed by the structure and return it.
    /// - If there is no recycled memory,
    ///   converts the newly freed physical memory to a Physical Page Number and returns it.
    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            // The next unassigned physical page number is pointed
            // to by incrementing it as it is assigned.
            self.current += 1;
            // change usize to PhisPageNum by use into()
            Some((self.current - 1).into())
        }
    }

    /// Push the ppn(PhysicalPageNumber) passed as argument to the `self.recycled` field.
    ///
    /// # Panic
    ///
    /// When recycling dealloc, the legitimacy of the recycled page must be verified
    /// before it can be held in the recycled stack.
    ///
    /// There are two conditions for a recycled page to be legitimate:
    ///
    /// - The page must have been previously assigned,<br/>
    ///   If ppn(PhysicalPageNumber) passed as an argument >= (self.current)the beginning of the unallocated address.
    /// -  If it is a double free,<br/>
    ///   i.e., its ppn(PhysicalPageNumber) passed as an argument is found on the `self.recycled` field.
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // validity check
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // recycle
        self.recycled.push(ppn);
    }
}

type FrameAllocatorImpl = StackFrameAllocator;

lazy_static! {
    /// frame allocator instance through lazy_static!
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        unsafe { UPSafeCell::new(FrameAllocatorImpl::new()) };
}

/// initiate the frame allocator using `ekernel` and `MEMORY_END`
pub fn init_frame_allocator() {
    extern "C" {
        /// The symbol is defined in `linker.ld`.
        /// - ekernel: end kernel memory segment
        fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access().init(
        // Round up the value to make `physical memory address` > a multiple of PAGE_SIZE(4096).
        PhysAddr::from(ekernel as usize).ceil(),
        // Truncate the value to make `physical memory address` <= a multiple of PAGE_SIZE(4096).
        PhysAddr::from(MEMORY_END).floor(),
    );
}

/// allocate a frame.
///
/// Change the physical address(range: `ekernel` symbol ~ `MEMORY_END`) => `PhysPageNum`
///
/// and wrap it in a `FrameTracker` to automatically call `frame_dealloc` when it is no longer used.
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

/// deallocate a frame
///
/// There is no need to use this function manually
/// since it is called automatically by `Drop` trait implemented in `FrameTracker`.
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

#[allow(unused)]
/// a simple test for frame allocator
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}
