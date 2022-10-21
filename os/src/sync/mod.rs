//! Synchronization and interior mutability primitives

mod mutex;
mod semaphore;
mod up;

pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::UPSafeCell;
