//!Implementation of [`PidAllocator`]
use crate::config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT_BASE, USER_STACK_SIZE};
use crate::mm::{MapPermission, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPIntrFreeCell;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use lazy_static::*;

use super::process::ProcessControlBlock;

/// Allocator to manage new ID assignment/reassignment/deletion, etc.
pub struct RecycleAllocator {
    /// Start position of unassigned id
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    ///Create an empty `PidAllocator`
    pub fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }

    ///Allocate a id(identifier)
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    ///Recycle a id(identifier)
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}

lazy_static! {
    static ref PID_ALLOCATOR: UPIntrFreeCell<RecycleAllocator> =
        unsafe { UPIntrFreeCell::new(RecycleAllocator::new()) };
    static ref KSTACK_ALLOCATOR: UPIntrFreeCell<RecycleAllocator> =
        unsafe { UPIntrFreeCell::new(RecycleAllocator::new()) };
}

pub const IDLE_PID: usize = 0;

/// Process ID handle
///
/// By wrapping the `Drop` in an implemented structure, memory can be automatically reclaimed
/// when it is no longer referenced from anywhere.
pub struct PidHandle(pub usize);

impl Drop for PidHandle {
    fn drop(&mut self) {
        //println!("drop pid {}", self.0);
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

///Allocate a pid(process identifier) from PID_ALLOCATOR
pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access().alloc())
}

/// Returns the (bottom, top) of the stack (kernel stack) allocated for each application's trap
/// processing in kernel space.
///
/// | Kernel address space |        |
/// |----------------------|--------|
/// |      trampoline      | --high |
/// |  app's KernelStack   |        |
/// |  app's guard page    |        |
/// |  app's KernelStack   |        |
/// |  app's guard page    | --low  |
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    // `+ PAGE_SIZE` is guard page
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

///KernelStack ID for each application
pub struct KernelStack(pub usize);

/// Allocate a new kernel ID and a new stack area in kernel space.
///
/// # Why?
/// Because each application requires its own stack for trap processing.
pub fn kstack_alloc() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.0);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}

impl KernelStack {
    #[allow(unused)]
    ///Push a value on top of kernelStack
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }

    /// Get the top address of the kernel stack.
    ///
    /// # Return
    /// The top address of the kernel stack
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.0);
        kernel_stack_top
    }
}

/// Individual thread resource(e.g. thread ID, user_stack, belongs process weak ref)
pub struct TaskUserRes {
    ///  Thread ID
    pub tid: usize,
    /// Base of stack address of the thread
    pub ustack_base: usize,
    /// Process to which the thread belongs
    pub process: Weak<ProcessControlBlock>,
}

/// Calculate and return the starting trap context address of each thread.
///
/// # Parameters
/// - `tid`: The thread ID
///
/// # Return
/// trap context base address for thread ID
///
/// # Example
///
/// ```rust
/// ustack_bottom_from_tid(ustack_base, 1);
/// ```
///
/// |  user address space | Size  |                              |
/// |---------------------|-------|------------------------------|
/// |  trap context base  |       | --high                       |
/// |  trap_cx of tid0    | 1page | __ Return this start address |
/// |  trap_cx of tid1    | 1page |                              |
/// |           ...       |       | __low                        |
fn trap_cx_bottom_from_tid(tid: usize) -> usize {
    // base + thread ID * previous thread trap_cx page
    TRAP_CONTEXT_BASE - tid * PAGE_SIZE
}

/// Calculate and return the starting stack address of each thread.
///
/// # Why is this function needed?
///
/// Threads share the address space of the process to which they belong, but since there is a separate
/// stack for the number of threads, this calculation is performed.
///
/// # Parameters
/// - `ustack_base`: The base address of the user stack
/// - `tid`: The thread ID
///
/// # Return
/// stack base address for thread ID
///
/// # Example
///
/// ```rust
/// ustack_bottom_from_tid(ustack_base, 1);
/// ```
///
/// |  user address space  |                              |
/// |----------------------|------------------------------|
/// |          ...         | --high                       |
/// |  UserStack of tid1   |                              |
/// |      guard page      | __ Return this start address |
/// |  UserStack of tid0   |                              |
/// |      guard page      |                              |
/// |   user stack base    | --low                       |
fn ustack_bottom_from_tid(ustack_base: usize, tid: usize) -> usize {
    // base + thread ID * Guard page of previous thread + stack size of previous thread
    ustack_base + tid * (PAGE_SIZE + USER_STACK_SIZE)
}

impl TaskUserRes {
    /// # Parameters
    /// - `process`: A thread is a process to which it belongs
    /// - `ustack_base`: Base address of the user stack for that thread
    /// - `alloc_user_res`: Allocate memory for thread ID resources (user stack and trap context)
    ///                     within the process to which it belongs?
    ///
    /// # Return
    /// Created Thread
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_base: usize,
        alloc_user_res: bool,
    ) -> Self {
        let tid = process.inner_exclusive_access().alloc_tid();
        let task_user_res = Self {
            tid,
            ustack_base,
            process: Arc::downgrade(&process),
        };
        if alloc_user_res {
            task_user_res.alloc_user_res();
        }
        task_user_res
    }

    /// Allocate memory for one thread's worth of resources (user stack and trap context)
    /// by thread ID within the process to which it belongs.
    pub fn alloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // alloc user stack
        let ustack_bottom = ustack_bottom_from_tid(self.ustack_base, self.tid);
        let ustack_top = ustack_bottom + USER_STACK_SIZE;
        process_inner.memory_set.insert_framed_area(
            ustack_bottom.into(),
            ustack_top.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );
        // alloc trap_cx
        let trap_cx_bottom = trap_cx_bottom_from_tid(self.tid);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        process_inner.memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );
    }

    /// Deallocate memory for one thread's worth of resources (user stack and trap context)
    /// by thread ID within the process to which it belongs.
    fn dealloc_user_res(&self) {
        // dealloc tid
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // dealloc ustack manually
        let ustack_bottom_va: VirtAddr = ustack_bottom_from_tid(self.ustack_base, self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());
        // dealloc trap_cx manually
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(trap_cx_bottom_va.into());
    }

    #[allow(unused)]
    /// Allocates a new thread ID to the process to which it belongs.
    pub fn alloc_tid(&mut self) {
        self.tid = self
            .process
            .upgrade()
            .unwrap()
            .inner_exclusive_access()
            .alloc_tid();
    }

    /// Deallocates a new thread ID for the process to which it belongs
    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }

    /// Calculate and return the starting trap context of user virtual address of this thread.
    ///
    /// # Return
    /// The base address of trap context  of current thread
    pub fn trap_cx_user_va(&self) -> usize {
        trap_cx_bottom_from_tid(self.tid)
    }

    /// Get the physical address number of the trap context for this thread ID.
    ///
    /// # Return
    /// The physical address number of the trap context for this thread ID
    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let process = self.process.upgrade().unwrap();
        let process_inner = process.inner_exclusive_access();
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn()
    }

    /// Get stack base address of this thread.
    ///
    /// # Return
    /// stack base address of this thread
    pub fn ustack_base(&self) -> usize {
        self.ustack_base
    }

    /// Get stack top address of this thread.
    ///
    /// # Return
    /// stack top address of this thread
    pub fn ustack_top(&self) -> usize {
        ustack_bottom_from_tid(self.ustack_base, self.tid) + USER_STACK_SIZE
    }
}

impl Drop for TaskUserRes {
    fn drop(&mut self) {
        self.dealloc_tid();
        self.dealloc_user_res();
    }
}
