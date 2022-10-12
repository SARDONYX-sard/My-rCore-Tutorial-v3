//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of [`PidAllocator`] called `PID_ALLOCATOR` allocates
//! pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod action;
mod context;
mod manager;
mod pid;
mod processor;
mod signal;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::{open_file, OpenFlags};
use alloc::sync::Arc;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use action::{SignalAction, SignalActions};
pub use context::TaskContext;
pub use manager::{add_task, pid2task};
pub use pid::{pid_alloc, KernelStack, PidAllocator, PidHandle};
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
pub use signal::{SignalFlags, MAX_SIG};

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of userTests app in make run TEST=1
pub const IDLE_PID: usize = 0;

#[cfg(feature = "board_qemu")]
use crate::board::QEMUExit;

use self::manager::remove_from_pid2task;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    #[cfg(feature = "board_qemu")]
    let pid = task.getpid();
    #[cfg(feature = "board_qemu")]
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        if exit_code != 0 {
            //crate::sbi::shutdown(255); //255 == -1 for err hint
            crate::board::QEMU_EXIT_HANDLE.exit_failure();
        } else {
            //crate::sbi::shutdown(0); //0 for success hint
            crate::board::QEMU_EXIT_HANDLE.exit_success();
        }
    }

    // remove from pid2task
    remove_from_pid2task(task.getpid());

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    ///Global process that init user shell
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(
    {
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    }
    );
}
///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}

/// If the signal representing the error is in the current task signals (self == SignalFlags)
/// => return (- signum, description)
pub fn check_signals_error_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    // println!(
    //     "[K] check_signals_error_of_current {:?}",
    //     task_inner.signals
    // );
    task_inner.signals.check_error()
}

/// Add a signal for the `signal` argument to the signals(`TaskBlockInner.signals`) waiting to be processed.
pub fn current_add_signal(signal: SignalFlags) {
    let inner = current_task().unwrap();
    let mut task_inner = inner.inner_exclusive_access();
    task_inner.signals |= signal;
}

/// Conditional branching depending on the signal of the `signal` argument
///
/// - `SIGSTOP` => set frozen to true, remove `SIGSTOP` from `task_inner.signals`
/// - If `SIGCONT` is in task_inner.signals => set frozen to false, remove `SIGCONT` from `task_inner.signals`.
/// - otherwise => set `task_inner.killed` to true
fn call_kernel_signal_handler(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            task_inner.frozen = true;
            task_inner.signals ^= SignalFlags::SIGSTOP;
        }
        SignalFlags::SIGCONT => {
            if task_inner.signals.contains(SignalFlags::SIGCONT) {
                task_inner.signals ^= SignalFlags::SIGCONT;
                task_inner.frozen = false;
            }
        }
        _ => {
            // println!(
            //     "[Kernel] call_kernel_signal_handler:: current task SignalFlag {:?}",
            //     task_inner.signals
            // );
            task_inner.killed = true;
        }
    }
}

/// Set the signal handler corresponding to the `sig` argument to sepc in the trap context.
/// # Parameters
/// - `sig`: Signal number. e.g. 9(SIGKILL)
/// - `signal`: Signals
fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();

    let handler = task_inner.signal_actions.table[sig].handler;

    // Is the handler function null ptr? (i.e., is the handler registered?)
    if handler != 0 {
        // user handler

        // change current mask
        task_inner.signal_mask = task_inner.signal_actions.table[sig].mask;
        // handle flag
        task_inner.handling_sig = sig as isize;
        // Assign the bit difference between the signal to be executed and the `signal` argument
        // to `task_inner.signals` using xor.
        task_inner.signals ^= signal;

        // backup trapframe
        let mut trap_ctx = task_inner.get_trap_cx();
        task_inner.trap_ctx_backup = Some(*trap_ctx);

        // modify trapframe
        // When returning from the kernel to the user state, instead of executing the code of the
        // user process before entering the kernel, the signal handler of that process is executed.
        // i.e.
        // The fact that it was put in sepc means that the jump destination
        // after the trap process is completed is the signal action handler.
        // - See `trap.S#106:108(csrw sepc, t1)`
        trap_ctx.sepc = handler;

        // put args (a0)
        trap_ctx.x[10] = sig;
    } else {
        // default action
        println!(
            "[Kernel] task/call_user_signal_handler: default action: ignore it or kill process"
        );
    }
}

/// Cycle through all signal numbers starting from 0 and process
///
/// - `SIGKILL`, `SIGSTOP`, `SIGCONT`, `SIGDEF` => call kernel signal handler
/// - otherwise => call user signal handler
fn check_pending_signals() {
    for sig in 0..(MAX_SIG + 1) {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        if task_inner.signals.contains(signal) && (!task_inner.signal_mask.contains(signal)) {
            drop(task_inner);
            drop(task);
            if signal == SignalFlags::SIGKILL
                || signal == SignalFlags::SIGSTOP
                || signal == SignalFlags::SIGCONT
                || signal == SignalFlags::SIGDEF
            {
                // signal is a kernel signal
                call_kernel_signal_handler(signal);
            } else {
                // signal is a user signal
                call_user_signal_handler(sig, signal);
                return;
            }
        }
    }
}

/// `frozen_flag` is true or `task_inner.killed` is false => It will continue to yield + loop
///
/// In the meantime, all signal numbers are cycled from 0 and the process associated with the signal is executed.
///
/// # Information
/// Currently this function is used when a trap occurs and returns from kernel space to user space.
pub fn handle_signals() {
    check_pending_signals();
    loop {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let frozen_flag = task_inner.frozen;
        let killed_flag = task_inner.killed;
        drop(task_inner);
        drop(task);
        // Has the task not been stopped and is the kill flag set?
        if (!frozen_flag) || killed_flag {
            break;
        }
        check_pending_signals();
        suspend_current_and_run_next()
    }
}
