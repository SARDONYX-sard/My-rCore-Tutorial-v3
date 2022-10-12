use crate::{sync::UPSafeCell, task::suspend_current_and_run_next};
use alloc::sync::{Arc, Weak};

use super::File;

/// Structure that stores information necessary to perform pipe processing
pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<UPSafeCell<PipeRingBuffer>>,
}

impl Pipe {
    /// Create a new `Pipe` with the following settings
    /// - readable: true
    /// - writable: false
    /// - buffer: first argument
    pub fn read_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }

    /// Create a new `Pipe` with the following settings
    /// - readable: false
    /// - writable: true
    /// - buffer: first argument
    pub fn write_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

const RING_BUFFER_SIZE: usize = 32;

#[derive(Copy, Clone, PartialEq, Eq)]
enum RingBufferStatus {
    /// Buffer is full and cannot be written
    Full,
    /// Buffer is empty and cannot be read
    Empty,
    /// Any state other than `FULL` and `EMPTY`
    Normal,
}

/// circular queue
pub struct PipeRingBuffer {
    /// Array of data
    arr: [u8; RING_BUFFER_SIZE],
    /// Index of head in the circular queue
    head: usize,
    /// Index of tail in the circular queue
    tail: usize,
    /// Full/Empty/Normal
    status: RingBufferStatus,
    /// A weak reference count for the write side of the pipe.
    ///
    /// This is because it may be necessary to verify that all write sides of the pipe have been closed,
    /// which can be easily checked in this field.
    write_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    /// Create a new ring buffer initialized with 0, `EMPTY` and `None`
    pub fn new() -> Self {
        Self {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: RingBufferStatus::Empty,
            write_end: None,
        }
    }

    /// Set a weak reference count for the write side of the pipe
    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }

    /// Writes `byte` to ring buffer.
    ///
    /// # Note
    /// Before calling this method, it must be ensured that the pipe buffer is not empty.
    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::Normal;
        self.arr[self.tail] = byte;
        // Do not exceed the max number of ring buffers.
        // if RING_BUFFER_SIZE is 32
        // 31 => 0
        // 32 => 1
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::Full;
        }
    }

    /// Reads byte from ring buffer.
    ///
    /// # Return
    /// Head of ring buffer
    ///
    /// # Note
    /// Before calling this method, it must be ensured that the pipe buffer is not empty.
    pub fn read_byte(&mut self) -> u8 {
        self.status = RingBufferStatus::Normal;
        let c = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        c
    }

    /// Calculates the number of characters still available in the pipe.
    pub fn available_read(&self) -> usize {
        // If the header and tail are equal, it means that the queue is empty or full,
        // and the return value of available_read is very different in each case,
        // so it is necessary to first determine if the queue is empty.
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + RING_BUFFER_SIZE - self.head
        }
    }

    /// Calculates the number of characters still available in the pipe.
    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }

    /// Determine if all write ends of the pipe are closed by attempting to upgrade
    /// the weak reference counts of the write ends held in the pipe to strong reference counts.
    pub fn all_write_ends_closed(&self) -> bool {
        // If the upgrade fails, the strong reference count on the write end of the pipe
        // will be zero, which means that all writes to the pipe have been completed,
        // and the pipe will not be refilled and may be destroyed when only the remaining data
        // in the pipe is read.
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Create a new `Pipe`.
///
/// Specifically, do the following
/// 1. Create a new ring buffer.
/// 2. Set the ring buffer to Pipe on the read and write sides.
/// 3. Set a weak reference of Pipe on the write side to the ring buffer side.
///
/// # Return
/// (read_end, write_end)
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(unsafe { UPSafeCell::new(PipeRingBuffer::new()) });
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    buffer.exclusive_access().set_write_end(&write_end);
    (read_end, write_end)
}

impl File for Pipe {
    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, buf: crate::mm::UserBuffer) -> usize {
        assert!(self.readable);
        let mut buf_iter = buf.into_iter();
        let mut read_size = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return read_size;
                }
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            // read at most loop_read bytes
            for _ in 0..loop_read {
                if let Some(byte_ref) = buf_iter.next() {
                    unsafe {
                        *byte_ref = ring_buffer.read_byte();
                    }
                    read_size += 1;
                } else {
                    return read_size;
                }
            }
        }
    }

    fn write(&self, buf: crate::mm::UserBuffer) -> usize {
        assert!(self.writable);
        let mut buf_iter = buf.into_iter();
        let mut write_size = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            // write at most loop_write bytes
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    write_size += 1;
                } else {
                    return write_size;
                }
            }
        }
    }
}
