//! ## A page table entry(64bit) in SV39 paging mode

use bitflags::*;

use super::PhysPageNum;

bitflags! {
    pub struct PTEFlags: u8 {
        /// Valid:
        /// - A page table entry is legal only if bit `V` is 1.
        const V = 1 << 0;
        /// Readable:
        /// - Controls whether the corresponding virtual page indexed
        ///   in this page table entry is allowed to read respectively.
        const R = 1 << 1;
        /// Writable:
        /// - Controls whether the corresponding virtual page indexed
        ///   in this page table entry is allowed to write respectively.
        const W = 1 << 2;
        /// Executable:
        /// - Controls whether the corresponding virtual page indexed
        ///   in this page table entry is allowed to execute respectively.
        const X = 1 << 3;
        /// User:
        /// - Controls whether access to the corresponding virtual page indexed
        ///   in this page table entry is allowed or not when the CPU has U privilege.
        const U = 1 << 4;
        /// Global:
        /// - Ignore for the time being.
        const G = 1 << 5;
        /// Accessed:
        /// - The processor records whether the virtual page corresponding to the page table entry
        ///   has been accessed since this bit on the page table entry was cleared.
        const A = 1 << 6;
        /// Dirty:
        /// - Indicates that a virtual page has been written since the last time the `D` bit was cleared.
        /// - The processor records whether the corresponding virtual page of the page table entry
        ///   has been modified since this bit on the page table entry was cleared.
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
///
/// The r/w/x flag if set to 0, meaning that the page entry points to the next page table.
///
/// ## A page table entry(64bit) in SV39 paging mode
///
/// | Bit number  |63      54|53    28  |27    19  |18    10  |9   8| 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
/// |-------------|----------|----------|----------|----------|-----|---|---|---|---|---|---|---|---|
/// | Bit meaning | Reserved | PPN\[2\] | PPN\[1\] | PPN\[0\] | RSW | D | A | G | U | X | W | R | V |
/// | Bit width   |    10    |    26    |     9    |     9    |  2  | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
///
/// - Reserved: The same bits as the last bit of the `PPN`(Physical Page Number)
///   are entered consecutively, otherwise it is an error.
/// - RSW: Reserved for supervisor software.
///        It is mentioned that RSW is left to the discretion of privileged software (i.e., the kernel)
///        and can be used, for example, to implement certain page swap algorithms.
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    /// generate an all-zero PageTableEntry,
    ///
    /// # Note
    ///
    /// This would be illegal because it would mean that the `V` flag bit of the PageTableEntry is zero.
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }

    ///  get Physical Page Number.
    pub fn ppn(&self) -> PhysPageNum {
        // PPN[2] PPN[1] PPN[0] is 10 ~ 53. width: 44bit
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
}

impl PageTableEntry {
    /// true if `V` flag is 1, false if it is 0.
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
}
