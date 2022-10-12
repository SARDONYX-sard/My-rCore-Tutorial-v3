//!Implementation of [`TaskManager`]
use super::task::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
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
    pub static ref PID2TCB: UPSafeCell<BTreeMap<usize, Arc<TaskControlBlock>>> =
        unsafe { UPSafeCell::new(BTreeMap::new()) };
}

/// Appends an element to the back of the deque.
pub fn add_task(task: Arc<TaskControlBlock>) {
    PID2TCB
        .exclusive_access()
        .insert(task.getpid(), Arc::clone(&task));
    TASK_MANAGER.exclusive_access().add(task);
}

///Interface offered to pop the first task
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

/// Get TaskControlBlock (one task data) from process ID
///
/// # Parameter
/// - `pid`: Process ID
///
/// # Return
/// - `TaskControlBlock` of pid
pub fn pid2task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    let map = PID2TCB.exclusive_access();
    map.get(&pid).map(Arc::clone)
}

/// Remove TaskControlBlock (one task data) from process ID
///
/// # Parameter
/// - `pid`: Process ID
///
/// # Panic
/// If there is no corresponding process ID.
pub fn remove_from_pid2task(pid: usize) {
    let mut map = PID2TCB.exclusive_access();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}
