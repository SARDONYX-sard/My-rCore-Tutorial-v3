//! Types related to task management
use super::id::TaskUserRes;
use super::{kstack_alloc, KernelStack, ProcessControlBlock, TaskContext};
use crate::mm::PhysPageNum;
use crate::sync::{UPIntrFreeCell, UPIntrRefMut};
use crate::trap::TrapContext;
use alloc::sync::{Arc, Weak};

/// A structure of the components of a single thread task
pub struct TaskControlBlock {
    // - immutable
    /// Reference to the process to which this thread belongs
    pub process: Weak<ProcessControlBlock>,
    /// Kernel stack assigned to a single application
    pub kstack: KernelStack,

    // - mutable
    /// Mutable information of the thread
    inner: UPIntrFreeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> UPIntrRefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    /// Get page table of the root node(user token) of the process to which this thread belongs
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
}

pub struct TaskControlBlockInner {
    /// Individual thread resource(e.g. thread ID, user_stack, belongs process weak ref)
    pub res: Option<TaskUserRes>,
    /// Physical page number of the physical page frame in the application address space
    /// where the thread's Trap context is located
    pub trap_cx_ppn: PhysPageNum,
    /// Thread context for threads interrupted for thread switching
    pub task_cx: TaskContext,
    /// Current thread execution status
    pub task_status: TaskStatus,
    /// Thread exit code(Number indicating the state of the thread when it is finished.)
    pub exit_code: Option<i32>,
}

impl TaskControlBlockInner {
    /// Get mutable TrapContext of this thread(self.trap_context_ppn field)
    ///
    /// # Return
    /// Mutable TrapContext of this thread
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    #[allow(unused)]
    /// Get current thread status(`self.task_status` field)
    ///
    /// # Return
    /// Current thread status
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
}

impl TaskControlBlock {
    /// Create a thread belonging to the process
    ///
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
        let res = TaskUserRes::new(Arc::clone(&process), ustack_base, alloc_user_res);
        let trap_cx_ppn = res.trap_cx_ppn();
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        Self {
            process: Arc::downgrade(&process),
            kstack,
            inner: unsafe {
                UPIntrFreeCell::new(TaskControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                })
            },
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
/// task status: Ready/Running/Blocking
pub enum TaskStatus {
    Ready,
    Running,
    Blocking,
}
