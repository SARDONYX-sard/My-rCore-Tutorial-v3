//! Types related to task management
use super::id::RecycleAllocator;
use super::manager::insert_into_pid2process;
use super::TaskControlBlock;
use super::{add_task, SignalFlags};
use super::{pid_alloc, PidHandle};
use crate::fs::{File, Stdin, Stdout};
use crate::mm::{translated_refmut, MemorySet, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

/// A structure of the components of a single task
pub struct ProcessControlBlock {
    // immutable
    pub pid: PidHandle,
    // mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

pub struct ProcessControlBlockInner {
    /// Is it in a zombie state (waiting for the process to finish running and be deleted)?
    pub is_zombie: bool,
    /// Address space for the application.
    pub memory_set: MemorySet,
    /// Parent of the current process (if it exists).
    /// Note:
    ///   This smart pointer does not affect the reference count of the parent process,
    ///   since we are wrapping another task control block using `Weak` instead of `Arc`.
    pub parent: Option<Weak<ProcessControlBlock>>,
    /// Instead, all task control blocks of the current process's children are held in the vector
    /// as `Arc` smart pointers so that they can be found more easily.
    pub children: Vec<Arc<ProcessControlBlock>>,
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
    /// Threads
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    /// Relatively generic resource allocator that can allocate process identifiers (PIDs) and thread KernelStacks.
    pub task_res_allocator: RecycleAllocator,
}

impl ProcessControlBlockInner {
    #[allow(unused)]
    /// Construct a u64-bit in satp CSR format with its paging mode as SV39
    /// and padding with the physical page number of the root node in the current multilevel page table.
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
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

    /// Allocate thread ID
    ///
    /// # Return
    /// Assigned thread ID
    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    /// Deallocate thread ID
    ///
    /// # Parameter
    /// - `tid` - Thread ID
    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    /// Get all threads number of this process
    ///
    /// # Return
    /// Number of threads
    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns thread information (`TaskControlBlock`) of the specified Thread ID.
    ///
    /// # Parameter
    /// - `tid` - Thread ID
    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);

        // allocate a pid
        let pid_handle = pid_alloc();
        // push a task context which goes to trap_return to the top of kernel stack
        let process = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
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
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                })
            },
        });

        // prepare TrapContext in user space
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );

        // add main thread to the process
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));

        // add main thread to scheduler
        add_task(task);
        process
    }

    /// execute elf
    ///
    /// # Parameters
    /// - `elf_data`: elf
    /// - `args`: command arguments
    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, mut ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        // substitute memory_set
        self.inner_exclusive_access().memory_set = memory_set;
        // then we alloc user resource for main thread again
        // since memory_set has been changed
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();

        // push arguments on user stack
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
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
                    new_token,
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
            ustack_base -= args[i].len() + 1;
            *argv[i] = ustack_base;
            let mut p = ustack_base;
            for c in args[i].as_bytes() {
                // Put 8 bits of data (1 character) into the current stack pointer(p).
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            // Put 0(ASCII '\0') into end of one command string
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8byte for k210 platform
        // Due to the different lengths of the command line arguments, pushing user_sp will likely
        // not align to 8 bytes and will cause an unaligned access exception when accessing the user
        // stack, so alignment adjustments should be made.
        ustack_base -= ustack_base % core::mem::size_of::<usize>();

        // initialize trap_cx
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_base,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        // x10 => user application 1st argument(a0)
        trap_cx.x[10] = args.len();
        // x11 => user application 2nd argument(a1)
        trap_cx.x[11] = argv_base;
        *task_inner.get_trap_cx() = trap_cx;
    }

    /// Fork this process
    ///
    /// # Return
    /// Forked process
    pub fn fork(self: &Arc<ProcessControlBlock>) -> Arc<ProcessControlBlock> {
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);

        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            //? Can't we just push without using `if let`?
            if let Some(fd) = fd {
                new_fd_table.push(Some(fd.clone()));
            } else {
                new_fd_table.push(None);
            }
        }

        let pid = pid_alloc();
        let child = Arc::new(ProcessControlBlock {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                })
            },
        });
        // add child
        parent.children.push(Arc::clone(&child));

        // create main thread of child process
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_base(),
            // here we do not allocate trap_cx or ustack again
            // but mention that we allocate a new kstack here
            false,
        ));

        // attach task process
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        drop(child_inner);

        // modify  kstack_top in trap_cx of this thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        drop(task_inner);
        insert_into_pid2process(child.getpid(), Arc::clone(&child));

        // add this thread to scheduler
        add_task(task);
        child
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}
