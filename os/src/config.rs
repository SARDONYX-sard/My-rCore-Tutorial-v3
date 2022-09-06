//! Constants used in rCore

/// 4096byte == 4KiB
pub const USER_STACK_SIZE: usize = 4096;
/// 4096 * 2 = 8KiB
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
/// 0x300000byte == 3MiB
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
/// 0x80800000 = 2.008GiB, 0x800000 = 8MiB
pub const MEMORY_END: usize = 0x80800000;
/// 4096byte == 4KiB
pub const PAGE_SIZE: usize = 0x1000;
/// Bit width of intra-page offset
pub const PAGE_SIZE_BITS: usize = 0xc;

/// Trampoline starting address
/// - usize::MAX - PAGE_SIZE + 1;
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// Trap Context starting address
/// - TRAMPOLINE - PAGE_SIZE
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/*
#[cfg(feature = "board_k210")]
pub const CLOCK_FREQ: usize = 403000000 / 62;
#[cfg(feature = "board_qemu")]
pub const CLOCK_FREQ: usize = 12500000;
*/
#[cfg(feature = "board_qemu")]
pub use crate::board::{CLOCK_FREQ, MMIO};
