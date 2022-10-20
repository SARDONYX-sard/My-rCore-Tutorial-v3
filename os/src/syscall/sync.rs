use crate::sync::{Mutex, MutexBlocking, MutexSpin, Semaphore};
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

/// Create a new exclusion control.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Parameter
/// - `res_count`:Number of threads with concurrent access to shared resources.
///
/// ## Counting(General) semaphores (`res_count` >= 2):
/// - Allow multiple threads with a maximum of `res_count` to access critical sections simultaneously
///
/// ## Binary semaphores(`res_count` == 1):
/// - Only one thread has access to the critical section.
/// - Semaphores restricted to values 0 and 1 (or locked/unlocked, disabled/enabled).
/// - Provide similar functionality to `Mutex`.
///
/// # Return
/// Index of the lock list within one process of the created `Semaphore`.
pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}

/// # V (Verhogen (Dutch), increase) operation
/// Increment semaphores(`self.count`)
///
///
/// If `self.count` is less than or equal to 0, a waiting thread is popped
/// from the top of the queue and added to the task queue (for the task to be executed).
///
/// # parameter
/// - `sem_id`: Semaphore ID(Index of the lock list within one process of the created `Semaphore`.)
///
/// # Return
/// always 0
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}

/// # P (Proberen (Dutch), try) operation
/// Decrement semaphores(`self.count`)
///
/// If `self.count` is less than 0, the currently running thread is added to the
/// end of `self.wait_queue` and continues waiting for the lock to be released in the `Blocking` state.
///
/// # parameter
/// - `sem_id`: Semaphore ID(Index of the lock list within one process of the created `Semaphore`.)
///
/// # Return
/// always 0
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    0
}
