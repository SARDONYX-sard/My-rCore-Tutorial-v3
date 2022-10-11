//! Process management syscalls
use crate::fs::{open_file, OpenFlags};
use crate::mm::{translated_ref, translated_refmut, translated_str};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next, pid2task,
    suspend_current_and_run_next, SignalAction, SignalFlags, MAX_SIG,
};
use crate::timer::get_time_ms;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// task exits and submit an exit code
///
/// # Parameters
/// - `exit_code`: The return value of the application.
///
/// # Panic
/// If the task cannot go to the next one as soon as it is finished.
pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

/// get time in milliseconds
pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

/// Get process identifier.
pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// Create a child process with a new address space that inherits the stack of the parent process.
/// The current process forks a child process.
///
/// # Return
/// - If child process => 0
/// - If current process => PID(Process Identifier) of child process
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0; //x[10] is a0 register
                       // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

/// Clear the address space of the current process, load a specific executable file,
/// return to the user state, and begin its execution.
///
/// # Parameter
/// - `path`: Name of the executable to load.
/// - `args`: Array of starting addresses for command line parameter strings.
///
/// # Return
/// Conditional branching.
/// - If there is an error => -1 (e.g. no executable file with matching name found)
/// - Otherwise => The length of `args` array
pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);

    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        // command line arguments are terminated?
        if arg_str_ptr == 0 {
            break;
        }
        args_vec.push(translated_str(token, arg_str_ptr as *const u8));
        unsafe {
            args = args.add(1);
        }
    }

    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        let argc = args_vec.len();
        task.exec(all_data.as_slice(), args_vec);
        // return argc because cx.x[10] will be covered with it later
        argc as isize
    } else {
        -1
    }
}

/// The current process waits for a child process to become a zombie process, collects all resources,
/// and collects its return value.
///
/// # Parameters
/// - `pid`: Process ID of the child process to wait. If -1, it means to wait for any child process.
/// - `exit_code_ptr`: Address where the return value of the child process is stored.
///              If this address is 0, it means that there is no need to store the return value.
///
/// # Return
/// Conditional branching.
/// - If there is not a child process whose pid is same as given => -1
/// - If there is a child process but it is still running => -2
/// - Otherwise => The process ID of the terminated child process
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

    // ---- access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB lock automatically
}

/// send a signal to the process
///
/// # Parameters
/// - `pid`: pid of the process
/// - `signal`: integer value representing the signal
///
/// # Return
/// Conditional branching.
/// - If the bit corresponding to `signum` in the signal of the process control block is successfully
///   set to 1. => 0
///
/// - No `TaskControlBlock` corresponding to `pid`(1st arg) => -1
/// - no `signal` corresponding to `signum` => -1
/// - If the bit of `signum` is already included in `signals` in the `TaskControlBlockInner`
///   corresponding to `pid` => -1
///
/// # Information
/// It is to send a signal with the value signum to the process with process number pid.
/// Specifically, it finds the process control block by `pid` and sets the bit corresponding to `signum`
/// in the signal of that process control block to 1.
pub fn sys_kill(pid: usize, signum: i32) -> isize {
    // Extract corresponding task from process ID.
    if let Some(task) = pid2task(pid) {
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            // insert the signal if legal
            let inner = task.inner_exclusive_access();
            let mut signals = inner.signals;
            if signals.contains(flag) {
                return -1;
            }
            signals.insert(flag);
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Set signal to block
///
/// # Parameters
/// - `mask`: signal mask
///
/// # Panic
/// When casting mask to SignalFlags fails.
///
/// # Return
/// Conditional branching.
/// - When `signal_mask` is rewritten successfully => old `signal_mask`
/// - Otherwise => -1
pub fn sys_sigprocmask(mask: u32) -> isize {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        let old_mask = inner.signal_mask;
        // ? Why not use `from_bits_truncates`?
        // ? - https://github.com/bitflags/bitflags/blob/main/src/traits.rs#L33
        if let Some(flag) = SignalFlags::from_bits(mask.try_into().unwrap()) {
            inner.signal_mask = flag;
            old_mask.bits() as isize
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Set the signal being processed to -1 (none) and restoring a backup of a trap context.
///
///  # Information
/// A recovery operation performed by the signal handler after it has finished responding to a signal,
/// i.e., restoring the trap context saved by the operating system before responding to the signal,
/// so that execution can continue from where the process was normally running before the signal was
/// processed.
///
/// # Return
/// Conditional branching.
/// - Success to restore a backup of a trap context => 0
/// - Otherwise => -1
pub fn sys_sigreturn() -> isize {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        inner.handling_sig = -1;
        // restore the trap context
        let trap_ctx = inner.get_trap_cx();
        *trap_ctx = inner.trap_ctx_backup.unwrap();
        0
    } else {
        -1
    }
}

/// - If `action` or `old_action` is 0,
/// - or `signum` is `SIGKILL(1<<9)` or `SIGSTOP(1<<19)` ?
fn check_sigaction_error(signal: SignalFlags, action: usize, old_action: usize) -> bool {
    action == 0
        || old_action == 0
        || signal == SignalFlags::SIGKILL
        || signal == SignalFlags::SIGSTOP
}

/// Registers a new handler (`action` argument) corresponding to the `signum` given as argument
/// and writes the original handler to `old_action`.
///
/// # Parameters
/// - `signum`: A signal bit digit corresponding to the process to be registered.
/// - `action`: new signal processing configuration
/// - `old_action`: old signal processing configuration
///
/// # Return
/// Conditional branching.
/// - If the `signum` and `action` arguments are successfully tied together => 0
///
/// - Failed to get the current task context => -1
/// - If `signum` exceeds the bit digits of `MAX_SIG` => -1
/// - If `action` or `old_action` is 0, or `signum` is `SIGKILL(1<<9)` or `SIGSTOP(1<<19)` => -1
pub fn sys_sigaction(
    signum: i32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    let token = current_user_token();
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        if signum as usize > MAX_SIG {
            return -1;
        }
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            if check_sigaction_error(flag, action as usize, old_action as usize) {
                return -1;
            }
            // 1. Store the address of the old signal handler in old_action.
            let old_kernel_action = inner.signal_actions.table[signum as usize];

            // - If `old_kernel_action` is `SIGILL(1<<4)` or `SIGABRT(1<<6)`?
            if old_kernel_action.mask != SignalFlags::from_bits(40).unwrap() {
                *translated_refmut(token, old_action) = old_kernel_action;
            } else {
                let mut ref_old_action = *translated_refmut(token, old_action);
                ref_old_action.handler = old_kernel_action.handler;
            }
            // 2. Save the address of the new signal_handler in the TCB signal_actions.
            let ref_action = translated_ref(token, action);
            inner.signal_actions.table[signum as usize] = *ref_action;
            return 0;
        }
    }
    -1
}
