//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].
mod context;

use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::syscall::syscall;
use crate::task::{
    check_signals_error_of_current, current_add_signal, current_trap_cx, current_user_token,
    exit_current_and_run_next, handle_signals, suspend_current_and_run_next, SignalFlags,
};
use crate::timer::set_next_trigger;
use core::arch::{asm, global_asm};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    set_kernel_trap_entry();
}

/// Write the `trap_from_kernel` address to the stvec(supervisor trap vector) register.
///
/// For horizontal trap(S-state -> S-state)
fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

/// Write the `TRAMPOLINE` address to the stvec(supervisor trap vector) register.
///
/// - `TRAMPOLINE` start address: of the springboard page shared by the kernel and application address space.
///
/// For vertical trap(U-state -> S-state)
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

/// handle an interrupt, exception, or system call from user space
/// Print trap exception.
///
/// This function is used in os/trap/trap.S __alltraps function
#[no_mangle]
pub fn trap_handler() -> ! {
    // If the S-state trap occurs again after entering the kernel,
    // the hardware skips the general-purpose register save process and jumps
    // to the trap_from_kernel function after setting some CSR registers, where it directly exits the panic.
    //
    // This is because the logic for saving and recovering the Trap context
    // is different for U-state→S-state and S-state→S-state
    // after the kernel and application address spaces are separated.
    // For simplicity, the S-state→S-state Trap process is weakened here, making it a straight panic.
    set_kernel_trap_entry();
    // Since the application's Trap context is not in the kernel address space,
    // call current_trap_cx to get a mutable reference to the current application's Trap context
    // instead of passing it as an argument to trap_handler as before.
    let scause = scause::read(); // get trap cause;
    let stval = stval::read(); // get extra value
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // get system call return value
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            // println!("[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.", stval, cx.sepc);
            // // page fault exit code
            // exit_current_and_run_next(-2);
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }

    // handle signals (handle the sent signal)
    handle_signals();
    // check error signals (if error then exit)
    if let Some((errno, msg)) = check_signals_error_of_current() {
        println!("[kernel] {}", msg);
        exit_current_and_run_next(errno);
    }

    // After processing the trap, call and return the user status.
    trap_return();
}

#[no_mangle]
/// set the new addr of __restore asm function in TRAMPOLINE page,
/// set the reg a0 = trap_cx_ptr, reg a1 = phy addr of usr page table,
/// finally, jump to new addr of __restore asm function
pub fn trap_return() -> ! {
    // We allow applications to jump to `__alltraps` when trapping to Supervisor state.
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    // Token in the application address space to continue execution.
    let user_satp = current_user_token();
    extern "C" {
        /// 1.Swap the TrapContext address in the `sscratch` register
        ///    with the User-stack address in the stack pointer.
        ///
        /// 2.Store the following in the TrapContext struct.
        /// - General-purpose registers in the current address space
        /// - `sstatus`
        /// - `sepc` register
        /// - `sepc`(address of app where trap occurred)
        ///
        /// 3.Read the following from the TrapContext struct.
        /// - Root page table of kernel
        /// - Address of trap_handler
        ///
        /// 4.Do the following.
        /// - Write the kernel root page table to the `satp` register
        /// - Jump to the address of trap_handler
        ///
        /// (This symbol is defined in "trap.S")
        fn __alltraps();
        fn __restore();
    }
    // `__alltraps` are aligned to TRAMPOLINE,
    // the starting address of the springboard page in address space.
    //
    //
    // - trampoline page(4096byte) in Application virtual address
    //
    // The highest memory address in Application virtual address
    //  ------------
    // | __restore  |
    //  ------------ <---100
    // | __alltraps |
    //  ------------ <--- 90 (TRAMPOLINE start address)
    //
    // The memory size of __alltraps(10) = __restore start(100) __alltraps start(90)
    // __restore(100) = The memory size of __alltraps(10) + TRAMPOLINE(90)
    //
    //
    // `__alltraps`: It's placed in the .text.trampoline segment(See `trap.S`).
    // springboard page: Page containing symbols for processing authority transitions (e.g. U->S state) during trap.
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        asm!(
            // clear instruction cache(i-cache) using `fence.i`.
            //
            // This is because the physical page frame that held the application code
            // may have been used for data or other application code,
            // and the i-cache may still hold an incorrect snapshot of that physical page frame.
            "fence.i",
            "jr {restore_va}",          // jump to new addr of `__restore` asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,      // a0 = virtual address of Trap Context
            in("a1") user_satp,        // a1 = physical address of usr page table
            options(noreturn)
        );
    }
}

#[no_mangle]
/// Unimplemented: traps/interrupts/exceptions from kernel mode
/// Todo: Chapter 9: I/O device
pub fn trap_from_kernel() -> ! {
    todo!("a trap from kernel!");
}

pub use context::TrapContext;
