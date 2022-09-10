//! Types related to task management
use core::cell::RefMut;

use super::pid::{KernelStack, PidHandle};
use super::TaskContext;
use crate::mm::{MemorySet, PhysPageNum};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

/// task control block structure
pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    // mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    /// Physical page frame number in the application address space where the Trap context is located.
    pub trap_cx_ppn: PhysPageNum,
    /// Means that the application data only exists in areas lower than the `base_size` byte of the
    /// application address space.
    ///
    /// This provides a clear picture of the amount of data present in the application memory.
    pub base_size: usize,
    /// Stores the context of the stopped task in the task control block.
    pub task_cx: TaskContext,
    /// Holds the current execution status of the process.
    pub task_status: TaskStatus,
    /// Address space for the application.
    pub memory_set: MemorySet,
    /// Parent of the current process (if it exists).
    // Note:
    //   This smart pointer does not affect the reference count of the parent process,
    //   since we are wrapping another task control block using `Weak` instead of `Arc`.
    pub parent: Option<Weak<TaskControlBlock>>,
    /// Instead, all task control blocks of the current process's children are held in the vector
    /// as `Arc` smart pointers so that they can be found more easily.
    pub children: Vec<Arc<TaskControlBlock>>,
    /// When a process exits spontaneously by invoking the exit system call or is terminated by the kernel
    /// with an error, its `exit_code` is stored in its task control block by the kernel and waits
    /// for the parent process to retrieve its PID and exit code while retrieving resources via `waitpid`.
    pub exit_code: i32,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    /// Construct a u64-bit in satp CSR format with its paging mode as SV39
    /// and padding with the physical page number of the root node in the current multilevel page table.
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    fn get_status(&self) -> TaskStatus {
        self.task_status
    }

    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn new(elf_data: &[u8]) -> Self {
        todo!()
    }

    pub fn exec(&self, elf_data: &[u8]) {
        todo!()
    }

    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        todo!()
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
/// task status: Ready, Running, Zombie
pub enum TaskStatus {
    Ready,
    Running,
    Zombie,
}
