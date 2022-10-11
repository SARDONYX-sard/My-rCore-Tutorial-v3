#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{exit, getpid, kill, sigaction, sigreturn, SignalAction, SignalFlags};

fn func() {
    println!("user_sig_test success");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    let mut new = SignalAction::default();
    let old = SignalAction::default();
    let sig_user_digit = SignalFlags::to_bit_digit(SignalFlags::SIGUSR1) as i32; // expect 10

    new.handler = func as usize;

    println!("signal_simple: sigaction");
    if sigaction(sig_user_digit, &new, &old) < 0 {
        panic!("Sigaction failed!");
    }
    println!("signal_simple: kill");
    if kill(getpid() as usize, sig_user_digit) < 0 {
        println!("Kill failed!");
        exit(1);
    }
    println!("signal_simple: Done");
    0
}
