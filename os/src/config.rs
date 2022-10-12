//! Constants used in rCore

/// 4096byte == 4KiB
pub const USER_STACK_SIZE: usize = 4096 * 2;
/// 4096 * 2 = 8KiB
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
/// 0x200000 byte == 2MiB
pub const KERNEL_HEAP_SIZE: usize = 0x20_0000;

/// 4096byte == 4KiB
pub const PAGE_SIZE: usize = 0x1000;
/// Bit width of intra-page offset
pub const PAGE_SIZE_BITS: usize = 0xc;

/// Trampoline(trap handler) starting virtual address
/// - usize::MAX - PAGE_SIZE + 1;
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// `TrapContext` starting virtual address
/// - TRAMPOLINE - PAGE_SIZE
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

#[cfg(feature = "board_qemu")]
pub use crate::board::{CLOCK_FREQ, MEMORY_END, MMIO};
