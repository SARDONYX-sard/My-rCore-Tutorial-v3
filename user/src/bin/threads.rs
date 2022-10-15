//! Multi-threaded application
//! - Threads begin execution by calling thread_create, which creates three threads
//! plus the main thread attached to the process, for a total of four threads. After
//! each thread outputs 1000 characters, it exits.
//! The process waits for these three threads to exit before finally terminating
//! the process via waittid.
#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use alloc::vec;
use user_lib::{exit, thread_create, waittid};

pub fn thread_a() -> ! {
    for _ in 0..1000 {
        print!("a");
    }
    exit(1)
}

pub fn thread_b() -> ! {
    for _ in 0..1000 {
        print!("b");
    }
    exit(2)
}

pub fn thread_c() -> ! {
    for _ in 0..1000 {
        print!("c");
    }
    exit(3)
}

#[no_mangle]
pub fn main() -> i32 {
    let v = vec![
        thread_create(thread_a as usize, 0),
        thread_create(thread_b as usize, 0),
        thread_create(thread_c as usize, 0),
    ];
    for tid in v.iter() {
        let exit_code = waittid(*tid as usize);
        println!("thread#{} exit code: {}", tid, exit_code);
    }
    println!("main thread exited.");
    0
}
