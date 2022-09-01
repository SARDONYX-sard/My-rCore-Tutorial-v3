//! Rust wrapper around `__switch`.
//!
//! Switching to a different task's context happens here. The actual
//! implementation must not be in Rust and (essentially) has to be in assembly
//! language (Do you know why?), so this module really is just a wrapper around
//! `switch.S`.

use super::context::TaskContext;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

// By having this function called instead of jumping directly to the address of the symbol __switch,
// the Rust compiler automatically inserts assembly code to save/restore the caller's save register
// before and after the call.
extern "C" {
    /// Switch to the context of `next_task_cx_ptr`, saving the current context
    /// in `current_task_cx_ptr`.
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
