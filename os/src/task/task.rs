//! Types related to task management
use core::cell::RefMut;

use super::pid::{pid_alloc, KernelStack, PidHandle};
use super::TaskContext;
use crate::config::TRAP_CONTEXT;
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};
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
    /// get mutable self.trap_context_ppn field
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    /// Construct a u64-bit in satp CSR format with its paging mode as SV39
    /// and padding with the physical page number of the root node in the current multilevel page table.
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    /// get `self.task_status` field
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }

    /// Is TaskStatus zombie?
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // push a task context which goes to trap_return to the top of kernel stack
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    pub fn exec(&self, elf_data: &[u8]) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        // **** access inner exclusively
        let mut inner = self.inner_exclusive_access();
        // substitute memory_set
        inner.memory_set = memory_set;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // initialize trap_cx
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // **** stop exclusively accessing inner automatically
    }

    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        // ---- access parent PCB exclusively
        let mut parent_inner = self.inner_exclusive_access();
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        });
        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx
        // **** access children PCB exclusively
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // return
        task_control_block
        // ---- stop exclusively accessing parent/children PCB automatically
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
