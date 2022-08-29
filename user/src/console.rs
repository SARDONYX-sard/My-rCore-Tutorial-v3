use core::fmt::{self, Write};

use crate::write;

const STDOUT: usize = 1;

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write(STDOUT, s.as_bytes());
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// Prints to the standard output.
///
/// Equivalent to the [`println!`] macro except that a newline is not printed at
/// the end of the message.
///
/// Use `print!` only for the primary output of your program.
///
/// [`println!`]: crate::println
///
/// # Examples
///
/// ```
/// print!("this ");
/// print!("will ");
/// print!("be ");
/// print!("on ");
/// print!("the ");
/// print!("same ");
/// print!("line ");
///
/// print!("this string has a newline, why not choose println! instead?\n");
/// ```
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $(arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(,$(arg)+)?))
    };
}

/// Prints to the standard output, with a newline.
///
/// Use `println!` only for the primary output of your program.
///
/// # Examples
///
/// ```
/// println!("hello there!");
/// println!("format {} arguments", "some");
/// let local_variable = "some";
/// println!("format {local_variable} arguments");
/// ```
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
