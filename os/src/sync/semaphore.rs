use crate::sync::UPSafeCell;
use crate::task::{add_task, block_current_and_run_next, current_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// Exclusion control, which allows multiple threads to access a resource simultaneously.
///
/// - While `Mutex` allows only one thread to access a critical section, Semaphore allows multiple
/// threads to access it.
///
/// # Example
/// ```rust
/// let semaphore = Semaphore::new(2);
/// ```
pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
}

/// inner for mutable exclusive control
pub struct SemaphoreInner {
    /// Maximum number of threads that can access the location where the critical section(Where thread conflicts occur)
    pub count: isize,
    /// Queue for waiting threads when the maximum number of threads accessible is exceeded(`self.count`).
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a semaphore.
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
    /// Created semaphore
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
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// # V (Verhogen (Dutch), increase) operation
    /// Increase `self.count` by 1.
    ///
    /// If `self.count` is less than or equal to 0, a waiting thread is popped
    /// from the top of the queue and added to the task queue (for the task to be executed).
    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                add_task(task);
            }
        }
    }

    /// # P (Proberen (Dutch), try) operation
    /// Decrease `self.count` by 1.
    ///
    /// If `self.count` is less than 0, the currently running thread is added to the
    /// end of `self.wait_queue` and continues waiting for the lock to be released in the `Blocking` state.
    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}
