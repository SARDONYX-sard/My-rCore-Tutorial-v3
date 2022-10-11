use crate::task::{SignalFlags, MAX_SIG};

/// Action for a signal
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    /// Address of the signal processing function corresponding to the signal
    pub handler: usize,
    /// Signal mask
    ///
    /// An attribute of a process, the list of signals that are blocked.
    ///
    /// SignalAction corresponding to the bit flags of the signal registered here will not be performed.
    pub mask: SignalFlags,
}

impl Default for SignalAction {
    /// Set null pointer in `self.handler`
    ///
    /// Set `SIGILL` (invalid instruction) and `SIGABRT` in `self.mask`.
    fn default() -> Self {
        Self {
            handler: 0,
            mask: SignalFlags::from_bits(40).unwrap(),
        }
    }
}

#[derive(Clone)]
/// Based on the contents of this array,
/// the OS can determine how the process should respond to the signal.
pub struct SignalActions {
    /// An array of SignalActions corresponding to each signal.
    pub table: [SignalAction; MAX_SIG + 1],
}

impl Default for SignalActions {
    fn default() -> Self {
        Self {
            table: [SignalAction::default(); MAX_SIG + 1],
        }
    }
}
