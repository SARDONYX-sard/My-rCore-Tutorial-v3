//! Types related to task management
use super::pid::{pid_alloc, KernelStack, PidHandle};
use super::{SignalActions, SignalFlags, TaskContext};
use crate::config::TRAP_CONTEXT;
use crate::fs::{File, Stdin, Stdout};
use crate::mm::{translated_refmut, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

/// A structure of the components of a single task
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
    /// File descriptor table
    ///
    /// To support file process management
    ///
    /// ## Option
    /// Whether the file descriptor is currently free or not.
    /// - Some=> occupied
    /// - None => free
    ///
    /// ## Arc
    /// Provides the ability to share references.
    /// - Multiple processes may share the same file for reading and writing.
    /// - The contents wrapped in `Arc` are placed on the kernel heap, not the stack,
    ///   so there is no need to specify the size at compile time.
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    /// A data type that holds a list of signal numbers
    ///
    /// Signals registered here are those that are to be processed.
    pub signals: SignalFlags,
    /// An attribute of a process, the list of signals that are blocked.
    ///
    /// `SignalAction` corresponding to the bit flags of the signal registered here will not be performed.
    pub signal_mask: SignalFlags,
    /// signal to be processed
    pub handling_sig: isize,
    /// List of signal handling routines
    pub signal_actions: SignalActions,
    /// whether the task was killed or not
    ///
    /// The purpose of kill is to flag whether the current process has been killed or not.
    /// This is so that the process is not terminated immediately upon receipt of the kill signal,
    /// but rather at the appropriate time.
    /// At this point, the kill is needed as a marker to exit the loop of unnecessary signal processing.
    pub killed: bool,
    /// whether the task is suspended
    ///
    /// Upon receipt of the signal
    /// - `SIGSTOP` => sets `frozen` to true, and the current process blocks to wait for `SIGCONT`.
    /// - `SIGCONT` => sets `frozen` to false, and the process exits
    ///                the cycle of waiting for the signal, returns to the user state, and continues execution.
    pub frozen: bool,
    /// Trap context in which the interruption occurred
    ///
    /// Necessary to return to the original process again after an interrupt by a signal is made.
    pub trap_ctx_backup: Option<TrapContext>,
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
    /// Search `self.fd_table` from the beginning to find `None` in the array.
    ///
    /// # Return
    /// Conditional branching.
    /// - Search `self.fd_table` from the beginning, and if `None` is already there => index of `None` found
    /// - If nothing found => push `None` to `self.fd_table` and index the last index of `self.fd_table`
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
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
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdin
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    signal_mask: SignalFlags::empty(),
                    handling_sig: -1,
                    signal_actions: SignalActions::default(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
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

    /// execute elf
    ///
    /// # Parameters
    /// - `elf_data`: elf
    /// - `args`: command arguments
    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, mut user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        // push arguments on user stack
        // args: ptr => [ptr of String, ptr of String...]
        // Therefore, multiply by usize (assuming 64), the size of the pointer,
        // to calculate the size of the pointer array (Vec) to be allocated on the stack.
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        // With argv_base as the starting address, get the physical address of each pointer
        // in the argv array and put it in Vector
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    memory_set.token(),
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();

        // Example(command aa bb)
        //
        // | HighAddr  |  byte |
        // |-----------|-------|<-- user_sp(original)
        // |     0     | 8byte |
        // |  argv[1]  | 8byte |
        // |  argv[0]  | 8byte |___ argv_base
        // |   '\0'    | 1byte |
        // |    'a'    | 1byte |
        // |    'a'    | 1byte |
        // |   '\0'    | 1byte |
        // |    'b'    | 1byte |
        // |    'b'    | 1byte |
        // | Alignment |  byte |
        // |-----------|-------|<-- user_sp(now)
        // |  LowAddr  |       |

        // Actually load arguments onto the stack.
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            // Shift the stack pointer by the length of the argument(command char)
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                // Put 8 bits of data (1 character) into the current stack pointer(p).
                *translated_refmut(memory_set.token(), p as *mut u8) = *c;
                p += 1;
            }
            // Put 0(ASCII '\0') into end of one command string
            *translated_refmut(memory_set.token(), p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8byte for k210 platform
        // Due to the different lengths of the command line arguments, pushing user_sp will likely
        // not align to 8 bytes and will cause an unaligned access exception when accessing the user
        // stack, so alignment adjustments should be made.
        user_sp -= user_sp % core::mem::size_of::<usize>();

        // **** access inner exclusively
        let mut inner = self.inner_exclusive_access();
        // substitute memory_set
        inner.memory_set = memory_set;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // initialize trap_cx
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // x10 => user application 1st argument(a0)
        trap_cx.x[10] = args.len();
        // x11 => user application 2nd argument(a1)
        trap_cx.x[11] = argv_base;
        *inner.get_trap_cx() = trap_cx;
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
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            //? Can't we just push without using `if let`?
            if let Some(fd) = fd {
                new_fd_table.push(Some(fd.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
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
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    signal_mask: parent_inner.signal_mask,
                    handling_sig: -1,
                    signal_actions: parent_inner.signal_actions.clone(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
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
