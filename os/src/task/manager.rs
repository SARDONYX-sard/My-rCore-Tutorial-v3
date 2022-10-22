//!Implementation of [`TaskManager`]
use super::{process::ProcessControlBlock, TaskControlBlock};
use crate::sync::UPSafeCell;
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use lazy_static::*;

pub struct TaskManager {
    /// Array of references to prepared tasks
    ///
    /// # Information
    /// Instead of putting the task control block directly into the TaskManager,
    /// place it on the kernel heap and store only the smart pointer of its reference count in the TaskManager,
    /// the unit of operation of the TaskManager, to reduce the overhead of data copying.
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

    /// Finds references in the ready_queue array that are identical to the `task` argument and removes them
    pub fn remove(&mut self, task: Arc<TaskControlBlock>) {
        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| Arc::as_ptr(t) == Arc::as_ptr(&task))
        {
            self.ready_queue.remove(id);
        }
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
    pub static ref PID2PCB: UPSafeCell<BTreeMap<usize, Arc<ProcessControlBlock>>> =
        unsafe { UPSafeCell::new(BTreeMap::new()) };
}

/// Appends an element to the back of the deque.
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

/// Remove an element to the back of the deque.
pub fn remove_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().remove(task);
}

///Interface offered to pop the first task
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

/// Insert into Map that ties process and process ID together
///
/// # Parameter
/// - `pid`: Process ID
/// - `process`: Process ID
///
/// # Return
/// - `ProcessControlBlock` of pid
pub fn insert_into_pid2process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2PCB.exclusive_access().insert(pid, process);
}

/// Get ProcessControlBlock (one process data) from process ID
///
/// # Parameter
/// - `pid`: Process ID
///
/// # Return
/// - `ProcessControlBlock` of pid
pub fn pid2process(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    let map = PID2PCB.exclusive_access();
    map.get(&pid).map(Arc::clone)
}

/// Remove ProcessControlBlock (one process data) from process ID
///
/// # Parameter
/// - `pid`: Process ID
///
/// # Panic
/// If there is no corresponding process ID.
pub fn remove_from_pid2process(pid: usize) {
    let mut map = PID2PCB.exclusive_access();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}
