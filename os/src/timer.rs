//! RISC-V timer-related functionality
use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use crate::sync::UPIntrFreeCell;
use crate::task::{add_task, TaskControlBlock};
use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use core::cmp::Ordering;
use lazy_static::*;
use riscv::register::time;

const TICKS_PER_SEC: usize = 100;
/// Pre-set clock frequency (Hertz) for each platform,
/// i.e., time interval for incrementing the counter in 1 second
const MSEC_PER_SEC: usize = 1000;

/// read the `mtime` register
pub fn get_time() -> usize {
    time::read()
}

/// get current time in milliseconds
pub fn get_time_ms() -> usize {
    get_time() / (CLOCK_FREQ / MSEC_PER_SEC)
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

pub struct TimerCondVar {
    pub expire_ms: usize,
    pub task: Arc<TaskControlBlock>,
}

impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}

impl Eq for TimerCondVar {}

impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let a = -(self.expire_ms as isize);
        let b = -(other.expire_ms as isize);
        Some(a.cmp(&b))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

lazy_static! {
    /// Timers set
    ///
    /// - [Why is Binary Heap Preferred over BST for Priority Queue?](https://www.geeksforgeeks.org/why-is-binary-heap-preferred-over-bst-for-priority-queue/?ref=rp)
    static ref TIMERS: UPIntrFreeCell<BinaryHeap<TimerCondVar>> =
        unsafe { UPIntrFreeCell::new(BinaryHeap::<TimerCondVar>::new()) };
}

/// Set a new timer
///
/// - `expire_ms`: Elapsed time until timer is triggered
/// - `task`: Tasks you want to enqueue when the time comes
pub fn add_timer(expire_ms: usize, task: Arc<TaskControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    timers.push(TimerCondVar { expire_ms, task });
}

/// If the current time is greater than the set deadline time, the associated task is added to the task queue.
///
/// This operation will while loop as long as the set timer exists
/// (however, the current OS uses time-division multitasking, so the task will be forced to switch after a certain time).
pub fn check_timer() {
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access();
    while let Some(timer) = timers.peek() {
        if timer.expire_ms <= current_ms {
            add_task(Arc::clone(&timer.task));
            timers.pop();
        } else {
            break;
        }
    }
}
