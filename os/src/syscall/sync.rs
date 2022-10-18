use crate::sync::{Mutex, MutexBlocking, MutexSpin};
use crate::task::current_process;
use alloc::sync::Arc;

/// Create a new exclusion control.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Parameter
/// - `blocking`: use `MutexBlocking`?
///
/// # Return
/// Index of the lock list within one process of the created Mutex.
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    // This `id` is index of memory array occupied for lock control in the past
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        // clone the vector
        .enumerate()
        // Find an available Lock mechanism
        .find(|(_, item)| item.is_none())
        // This `id` is index of memory array occupied for lock control in the past too.
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}

/// **Lock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **lock**
///
/// # Return
/// always 0
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}

/// **Unlock** the `Mutex` of the index specified by the argument from the lock management list (`self.mutex_list`)
/// existing in the currently running process
///
/// # Parameter
/// - `mutex_id`: Mutex index you want to **unlock**
///
/// # Return
/// always 0
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
