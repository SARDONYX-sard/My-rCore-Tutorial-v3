//! Types related to task management

use super::TaskContext;

#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
}

// see: https://github.com/rust-lang/rust-clippy/issues/9063
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
