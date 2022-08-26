use core::panic::PanicInfo;

use crate::sbi::shutdown;

/// Prints to the standard output, with a newline
/// and shutdown.
///
/// # Examples
///
/// ```
/// panic!("Shutdown machine!");
/// // >Panicked at src/main.rs:16 Shutdown machine!
/// ```
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        println!("Panicked: {}", info.message().unwrap())
    }
    shutdown()
}
