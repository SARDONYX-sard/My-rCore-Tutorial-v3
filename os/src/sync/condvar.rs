use crate::sync::{Mutex, UPIntrFreeCell};
use crate::task::{
    add_task, block_current_and_run_next, block_current_task, current_task, TaskContext,
    TaskControlBlock,
};
use alloc::{collections::VecDeque, sync::Arc};

/// # Exclusive control by Conditional variable
///
/// The internal implementation is similar to semaphore.
///
/// A system that performs exclusion control by having a common resource (e.g., static variable) call
/// wait on one thread during certain conditions and having the other thread satisfy the conditions and call signal.
pub struct Condvar {
    pub inner: UPIntrFreeCell<CondvarInner>,
}

/// inner for mutable exclusive control
pub struct CondvarInner {
    /// Queue for waiting threads.
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Condvar {
    /// Create a Conditional variable.
    ///
    /// # Return
    /// Created Condvar
    ///
    /// # Example
    /// ```rust
    /// let condvar = Condvar::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPIntrFreeCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// Takes one thread from the head of the waiting thread queue and adds it to the task queue.
    ///
    /// By resuming the thread with this method, the **`lock`** method of `Mutex` given the
    /// `Condvar.wait` method is finally called.
    pub fn signal(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(task) = inner.wait_queue.pop_front() {
            add_task(task);
        }
    }

    // pub fn wait(&self, mutex: Arc<dyn Mutex>) {
    //     mutex.unlock();
    //     let mut inner = self.inner.exclusive_access();
    //     inner.wait_queue.push_back(current_task().unwrap());
    //     drop(inner);
    //     block_current_and_run_next();
    //     mutex.lock();
    // }

    pub fn wait_no_sched(&self) -> *mut TaskContext {
        self.inner.exclusive_session(|inner| {
            inner.wait_queue.push_back(current_task().unwrap());
        });
        block_current_task()
    }

    /// Wait until the lock is obtained in the following order.
    ///
    /// 1. call the **`unlock`** method of `Mutex` given as the `mutex` argument.
    ///
    /// 2. add the currently running thread to the end of the waiting thread queue,
    ///    and keep that thread waiting with blocking.
    /// <br>
    /// 3. **When it is added to the task queue by `Condvar.signal`**,
    ///    finally call the **`lock`** method of `Mutex` given as the `mutex` argument.
    pub fn wait_with_mutex(&self, mutex: Arc<dyn Mutex>) {
        mutex.unlock();
        self.inner.exclusive_session(|inner| {
            inner.wait_queue.push_back(current_task().unwrap());
        });
        block_current_and_run_next();
        mutex.lock();
    }
}
