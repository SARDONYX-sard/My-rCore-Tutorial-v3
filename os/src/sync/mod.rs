//! Synchronization and interior mutability primitives

mod mutex;
mod up;

pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use up::UPSafeCell;
