#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

extern crate alloc;

#[macro_use]
extern crate user_lib;

/// LF line feed [\n](https://www.barcodefaq.com/ascii-chart-char-set/)
const LF: u8 = 0x0au8;
/// CR carriage return [\r](https://www.barcodefaq.com/ascii-chart-char-set/)
const CR: u8 = 0x0du8;
/// Keyboard keycode: Delete
const DL: u8 = 0x7fu8;
/// Keyboard keycode: BackSpace
const BS: u8 = 0x08u8;

use alloc::string::String;
use alloc::vec::Vec;
use user_lib::console::getchar;
use user_lib::{exec, fork, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    // The &str of the args after the split is the subInterval of the line that contains not \0 at the end.
                    let args: Vec<_> = line.as_str().split(' ').collect();
                    let mut args_copy: Vec<String> = args
                        .iter()
                        .map(|&arg| {
                            let mut string = String::new();
                            string.push_str(arg);
                            string
                        })
                        .collect();
                    // line is our input, and there is no not \0 in the middle.
                    // When we pass it to the kernel, we can only pass the first address of the string,
                    // so we must make sure it ends in \0.
                    args_copy.iter_mut().for_each(|string| {
                        // From there we use args_copy to copy the args string to the heap
                        // and manually add the trailing \0.
                        string.push('\0');
                    });

                    let mut args_addr: Vec<*const u8> =
                        args_copy.iter().map(|arg| arg.as_ptr()).collect();
                    // Each element of the args_addr vector represents the starting address of a command line argument string.
                    //
                    // It is the starting address of this vector that is passed to the kernel,
                    // so in order for the kernel to get the number of command line arguments,
                    // args_addr must end with a zero so that the kernel knows that the command line arguments have been taken
                    // when it sees them.
                    args_addr.push(core::ptr::null::<u8>());

                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(line.as_str(), args_addr.as_slice()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
