use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;

/// Sleep for the milliseconds given in the `period_ms` argument.
///
/// # Parameter
/// - `ms`: Milliseconds to sleep
pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

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
/// ## Semaphore for synchronization purpose(`res_count` == 0):
/// - If 0, calling up will always add to the task queue, and calling down will always cause the thread to wait.
///   This mechanism allows synchronization of common variables of threads.
///
/// # Return
/// Index of the lock list within one process of the created `Semaphore`.
///
/// # Example
/// ```rust
/// /// As `Counting Semaphores`
/// let semaphore = Semaphore::new(2);
///
/// /// As `Mutex`
/// let mutex_semaphore = Semaphore::new(1);
///
/// /// For `Sync Threads`
/// let mutex_semaphore = Semaphore::new(0);
/// ```
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

/// Create Exclusive Control with Conditional Variable.
/// - If there is an existing memory area for the old lock => reuse it and return its index
/// - If not exist => push a new one and return its index
///
/// # Parameter
/// - `_arg`: unused value
///
/// # Return
/// Index of the lock list within one process of the created `Condvar`.
pub fn sys_condvar_create(_arg: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    // Reuse condvar
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

/// Takes one thread from the head of the waiting thread queue and adds it to the task queue.
///
/// By resuming the thread with this method, the **`lock`** method of `Mutex` given the
/// `Condvar.wait` method is finally called.
///
/// # parameter
/// - `condvar_id`: Condvar ID(Index of the lock list within one process of the created `Condvar`.)
///
/// # Return
/// Always 0
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

/// Wait until the lock is obtained in the following order.
///
/// 1. call the **`unlock`** method of `Mutex` given as the `mutex` argument.
///
/// 2. add the currently running thread to the end of the waiting thread queue,
///    and keep that thread waiting with blocking.
/// <br>
/// 3. **When it is added to the task queue by `Condvar.signal`**,
///    finally call the **`lock`** method of `Mutex` given as the `mutex_id` argument.
///
/// # parameters
/// - `condvar_id`: Condvar ID(Index of the lock list within one process of the created `Condvar`.)
/// - `mutex_id`: Mutex ID(Index of the lock list within one process of the created `Mutex`.)
///
/// # Return
/// Always 0
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait_with_mutex(mutex);
    0
}
