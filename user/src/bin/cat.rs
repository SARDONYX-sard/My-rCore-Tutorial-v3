#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use alloc::string::String;
use user_lib::{close, open, read, OpenFlags};

/// Opens the file specified by the fileName command argument, reads buf (8bit * 16)
/// and outputs the characters to standard output.
///
/// # Command usage
/// cat \<fileName\>
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    assert_eq!(argc, 2);
    let fd = open(argv[1], OpenFlags::RDONLY);
    if fd == -1 {
        panic!("Error occurred when opening file");
    }

    let fd = fd as usize;
    let mut buf = [0u8; 16];
    let mut s = String::new();
    loop {
        let size = read(fd, &mut buf) as usize;
        if size == 0 {
            break;
        }
        s.push_str(core::str::from_utf8(&buf[..size]).unwrap());
    }
    println!("{}", s);
    close(fd);
    0
}
