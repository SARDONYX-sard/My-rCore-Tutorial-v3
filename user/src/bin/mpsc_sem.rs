#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

#[macro_use]
extern crate user_lib;

extern crate alloc;

use alloc::vec::Vec;
use user_lib::exit;
use user_lib::{semaphore_create, semaphore_down, semaphore_up};
use user_lib::{thread_create, waittid};

/// Semaphore ID(Semaphore init_value: 1, range: 0 ~ 1)
const SEM_MUTEX: usize = 0;
/// Semaphore ID(Semaphore value: 8 as `BUFFER_SIZE`)
const SEM_EMPTY: usize = 1;
/// Semaphore ID(Semaphore value: 0)
const SEM_EXISTED: usize = 2;
/// Semaphore value
const BUFFER_SIZE: usize = 8;
/// This buffer is ring buffer
static mut BUFFER: [usize; BUFFER_SIZE] = [0; BUFFER_SIZE];
/// The head of ring buffer. range: 0 ~ BUFFER_SIZE(8)
static mut FRONT: usize = 0;
/// The tail of ring buffer. range: 0 ~ BUFFER_SIZE(8)
static mut TAIL: usize = 0;
/// The number of threads
const PRODUCER_COUNT: usize = 4;
const NUMBER_PER_PRODUCER: usize = 100;

// Because of the assignment to the static mut variable, the unsafe function
unsafe fn producer(id: *const usize) -> ! {
    let id = *id;
    for _ in 0..NUMBER_PER_PRODUCER {
        semaphore_down(SEM_EMPTY); // 1st producer: 8 + 1 -> - 92, 2nd: -93 -> -192, 3rd: -193 ~ -202, 4th: -203 ~ -302
        semaphore_down(SEM_MUTEX); // set 0 == lock
        BUFFER[FRONT] = id;
        FRONT = (FRONT + 1) % BUFFER_SIZE;
        semaphore_up(SEM_MUTEX); // set 1 == unlock
        semaphore_up(SEM_EXISTED); // 1st: 0 + 1 -> 99, 2nd: 100 -> 199, 3rd: 200 ~ 299, 4th: 300 ~ 399
    }
    exit(0)
}

// Because of the assignment to the static mut variable, the unsafe function
unsafe fn consumer() -> ! {
    for _ in 0..PRODUCER_COUNT * NUMBER_PER_PRODUCER {
        semaphore_down(SEM_EXISTED); // 399 -> 0
        semaphore_down(SEM_MUTEX);
        print!("{} ", BUFFER[TAIL]);
        TAIL = (TAIL + 1) % BUFFER_SIZE;
        semaphore_up(SEM_MUTEX);
        semaphore_up(SEM_EMPTY); // -302 -> 8
    }
    println!("");
    exit(0)
}

#[no_mangle]
pub fn main() -> i32 {
    // create semaphores
    assert_eq!(semaphore_create(1) as usize, SEM_MUTEX);
    assert_eq!(semaphore_create(BUFFER_SIZE) as usize, SEM_EMPTY);
    assert_eq!(semaphore_create(0) as usize, SEM_EXISTED);
    // create threads
    let ids: Vec<_> = (0..PRODUCER_COUNT).collect();
    let mut threads = Vec::new();
    for i in 0..PRODUCER_COUNT {
        threads.push(thread_create(
            producer as usize,
            &ids.as_slice()[i] as *const _ as usize,
        ));
    }
    threads.push(thread_create(consumer as usize, 0));
    // wait for all threads to complete
    for thread in threads.iter() {
        waittid(*thread as usize);
    }
    println!("mpsc_sem passed!");
    0
}
