use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{add_task, current_task};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use alloc::{collections::VecDeque, sync::Arc};

/// Exclusion control mechanism for safe data modification under multi-threading.
///
/// # Example
/// ```rust
/// impl Mutex for MutexSample {
///     pub fn lock(&self) {
///         todo!()
///     }
///
///     pub fn unlock(&self) {
///         todo!()
///     }
/// }
/// ```
pub trait Mutex: Sync + Send {
    /// If no other thread has a lock, acquire a lock.
    ///
    /// Otherwise, `yield` (turn other tasks) until a lock is obtained.
    ///
    /// # Example
    /// ```rust
    /// use crate::sync::{Mutex, MutexBlocking, MutexSpin};
    /// use crate::task::current_process;
    /// use alloc::sync::Arc;
    ///
    /// let mutex_id = 1;
    /// let process = current_process();
    /// let process_inner = process.inner_exclusive_access();
    /// let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    /// drop(process_inner);
    /// drop(process);
    /// mutex.lock();
    /// ```
    fn lock(&self);
    /// Set the `self.locked` flag to false and let go of the lock.
    ///
    /// # Example
    /// ```rust
    /// use crate::sync::{Mutex, MutexBlocking, MutexSpin};
    /// use crate::task::current_process;
    /// use alloc::sync::Arc;
    ///
    /// let mutex_id = 1;
    /// let process = current_process();
    /// let process_inner = process.inner_exclusive_access();
    /// let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    /// drop(process_inner);
    /// drop(process);
    /// mutex.unlock();
    /// ```
    fn unlock(&self);
}

/// # Mutex(Exclusive control of lock acquisition competition)
///
/// When multiple threads are running in concurrency on one core and one thread is getting a lock,
///
/// the other threads will just keep running `yield` until the lock is obtained.
///
/// # Differences from `MutexBlocking`
/// - `MutexSpin` does not consider fairness in acquiring locks.
/// - Race to acquire locks occurs.
///
/// # Figure
/// | threads |               state                |
/// |---------|------------------------------------|
/// | thread1 |              locked                |
/// | thread2 | loop `yield` until thread1 unlocks |
/// | thread3 | loop `yield` until thread1 unlocks |
/// | thread4 | loop `yield` until thread1 unlocks |
pub struct MutexSpin {
    /// Exclusive variable lock flag
    ///
    /// Currently locked?
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new structure with `self.locked` flag **false**.
    ///
    /// # Example
    /// ```rust
    /// let mutex = MutexSpin::new();
    /// ```
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// # Mutex(Fair Exclusion Control)
///
/// When multiple threads are running in concurrency on one core and one thread is getting a lock,
/// threads that cannot obtain a lock are stored in the queue for threads waiting for a lock,
/// and as soon as a lock can be obtained, the waiting thread at the head of the queue is popped, and that thread obtains the lock.
///
/// # Figure
/// | threads |                     state                     |                                                                             |
/// |---------|-----------------------------------------------|-----------------------------------------------------------------------------|
/// | thread1 |                    locked                     |                                                                             |
/// | thread2 | state `Blocking` and push_back to wait queue  | <-The next one to take the lock is this thread, who was in the first queue. |
/// | thread3 | state `Blocking`  and push_back to wait queue |                                                                             |
/// | thread4 | state `Blocking`  and push_back to wait queue |                                                                             |
pub struct MutexBlocking {
    /// Structure with variable fields storing locked, wait_queue(for thread)
    inner: UPSafeCell<MutexBlockingInner>,
}

/// inner for mutable exclusive control
pub struct MutexBlockingInner {
    /// Exclusive variable lock flag
    ///
    /// Currently locked?
    locked: bool,
    /// Wait queue to hold threads waiting for locks
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new structure with `self.locked` flag **false**.
    ///
    /// # Example
    /// ```rust
    /// let mutex = MutexBlocking::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            add_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
}
