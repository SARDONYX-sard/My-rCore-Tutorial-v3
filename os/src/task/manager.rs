//!Implementation of [`TaskManager`]
use super::task::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::{collections::VecDeque, sync::Arc};
use lazy_static::*;

pub struct TaskManager {
    // Instead of putting the task control block directly into the TaskManager,
    // place it on the kernel heap and store only the smart pointer of its reference count in the TaskManager,
    // the unit of operation of the TaskManager, to reduce the overhead of data copying.
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Create an empty `TaskManager`
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }

    ///Add a task to `TaskManager`
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    ///Remove the first task and return it,or `None` if `TaskManager` is empty
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Appends an element to the back of the deque.
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

///Interface offered to pop the first task
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
