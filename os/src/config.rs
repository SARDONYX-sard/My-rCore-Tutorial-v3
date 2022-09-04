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
pub const PAGE_SIZE: usize = 4096;
/// Bit width of intra-page offset
pub const PAGE_SIZE_BITS: usize = 12;

pub const MAX_APP_NUM: usize = 4;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;
/*
#[cfg(feature = "board_k210")]
pub const CLOCK_FREQ: usize = 403000000 / 62;
#[cfg(feature = "board_qemu")]
pub const CLOCK_FREQ: usize = 12500000;
*/
#[cfg(feature = "board_qemu")]
pub use crate::board::CLOCK_FREQ;
