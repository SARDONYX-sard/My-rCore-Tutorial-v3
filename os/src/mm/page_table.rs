//! ## A page table entry(64bit) in SV39 paging mode

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

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
/// # Page table entry(64bit)
///
/// `usize` memory to store physical number(PPN) and access control information.
///
/// ## Memory specification in SV39 paging mode
///
/// | Bit number  |63------54|53------28|27------19|18------10|9---8| 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
/// |-------------|----------|----------|----------|----------|-----|---|---|---|---|---|---|---|---|
/// | Bit meaning | Reserved | PPN\[2\] | PPN\[1\] | PPN\[0\] | RSW | D | A | G | U | X | W | R | V |
/// | Bit width   |    10    |    26    |     9    |     9    |  2  | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
///
/// - Reserved: The same bits as the last bit of the `PPN`(Physical Page Number)
///   are entered consecutively, otherwise it is an error.
/// - RSW: Reserved for supervisor software.
///        It is mentioned that RSW is left to the discretion of privileged software (i.e., the kernel)
///        and can be used, for example, to implement certain page swap algorithms.
///
/// The v flag set to 1 and the r/w/x flag if set to 0,
/// meaning that the (PPN)PhysicalPageNumber points to the next page table.
///
/// Layers of page tables are called multi-level page tables.
///
/// Then use the virtual page number as an index to obtain the next page table
/// or a page to a physical address.
///
/// See more: [4.4.1 Addressing and Memory Protection](https://five-embeddev.com/riscv-isa-manual/latest/supervisor.html#addressing-and-memory-protection)
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

    /// true if `V` flag is 1, false if it is 0.
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }

    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }

    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }

    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// # Page table
///
/// Since each application address space corresponds to a different multi-level page table,
/// the starting address (i.e., the address of the root node of the page table)
/// will be different for each page table.
///
/// Therefore, the PageTable keeps the `root_ppn`, which is the physical page number of its root node,
/// as a marker to uniquely distinguish the page table.
///
/// # What is different from PageTableEntry?
///
/// This PageTable struct is for grouping page tables by application.
pub struct PageTable {
    /// Physical page number
    ///
    /// SV39: 56(PhisAddr) - 12(offset) = 44bit
    root_ppn: PhysPageNum,
    /// The physical page frames of all nodes of the PageTable (including the root node)
    /// are held in the form of FrameTrackers.
    ///
    /// # Information
    ///
    /// This is in line with the test procedure of the Physical Page Frame Management module,
    /// and the lifecycle of these FrameTrackers is further bound to the PageTable.
    ///
    /// When the lifecycle of the PageTable ends, those FrameTrackers in the vector frame are also recycled,
    /// which means that the physical page frame holding the multi-level PageTable node is recycled.
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    /// Create a new PageTable with the value of the argument satp
    /// (Supervisor Address Translation and Protection) register as root_node.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    /// Finds and returns the `PageTableEntry` associated with the vpn.
    ///
    /// If the searched `PageTableEntry` is the terminal node, the value is returned.
    /// Otherwise, return `None`.
    ///
    /// - If the Valid flag of the found `PageTableEntry` is 0,
    /// it is overwritten by a new `PageTableEntry` with the Valid flag set to 1.<br/>
    /// Then add to frames(vector in node tracking for each app).
    ///
    /// # Details
    ///
    /// The vpn (VirtualPageNumber): 27bit(SV39) given as an argument
    /// => divide \[VPN0\](9bit), VPN\[1\](9bit), VPN\[2\](9bit)\]
    ///
    /// And each part is used as an index to search the PageTable of each layer.
    /// - VPN\[0\]: The index of 3rd level page table.
    /// - VPN\[1\]: The index of 2nd level page table.
    /// - VPN\[2\]: The index of 1st level page table.
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            // Get page table and use 9 bits(Max:512) of virtual page number as index.
            // What you get at this point is the next `PageTableEntry`.
            //
            // That is, i is 0
            // - When i is 0, it is the 2nd level page table.
            // - when i is 1, it is the 1st level page table.
            // - When it is 2, it is the actual physical address number
            //   (combining this with the offset, the physical address is obtained).
            let pte = &mut ppn.get_pte_array()[*idx];
            // is level 1 table?
            if i == 2 {
                // Physical page number stored in 1st level page,
                // which refers to `PageTableEntry`
                // to the physical address that is the terminal node.
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    /// Finds and returns the `PageTableEntry` associated with the vpn.
    ///
    /// If the searched `PageTableEntry` is the terminal node, the value is returned.
    /// Otherwise, return `None`.
    ///
    /// - If the Valid flag of the found `PageTableEntry` is 0,
    ///   return `None`.
    ///
    /// # Details
    ///
    /// The vpn (VirtualPageNumber): 27bit(SV39) given as an argument
    /// => divide \[VPN0\](9bit), VPN\[1\](9bit), VPN\[2\](9bit)\]
    ///
    /// And each part is used as an index to search the PageTable of each layer.
    /// - VPN\[0\]: The index of 3rd level page table.
    /// - VPN\[1\]: The index of 2nd level page table.
    /// - VPN\[2\]: The index of 1st level page table.
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// Combining the physical number and access flags creates a page table entry.
    ///
    /// Mapping to that table using the virtual page number as a key
    ///
    ///  # The TLB is not refreshed after mapping and unmapping.
    ///
    /// Since the application and the kernel are in different address spaces,
    /// there is no need to refresh the TLB immediately after each map/unmap,
    /// but only once after all operations and before returning to the application address space.
    ///
    ///  The reason for this is that refreshing the TLB is a very time-consuming operation,
    /// and unnecessary refresh should be avoided whenever possible,
    /// so the TLB is not refreshed after every map and unmap.
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        //? INFO: The current implementation does not intend to do anything about running out of physical page frames
        //?       but just panic out. So you can see a lot of unwrap in the preceding code,
        //?       which is not recommended by Rust, but just for simplicity's sake.
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    #[allow(unused)]
    /// Finds the page table entry from the virtual page number passed as an argument
    /// and fills it with zero.
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }

    /// `PageTableEntry` with the physical address number of the terminal node
    /// from the argument vpn, or `None` if not found.
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    /// Get the physical page number of the root node of that application.
    ///
    /// Physical page number(SV39: 44bit)
    pub fn token(&self) -> usize {
        // 8 = 0b1000
        // 0b1000 << 60 = 1 and (3 + 60)-digit zero.
        // This is a total of 64 bits.
        //
        // The 64th digit is 1, but since it is the last 44 bits that are used,
        // there is no need to be concerned.
        8usize << 60 | self.root_ppn.0
    }
}

/// Temporarily create a `PageTable` with token as root_node
/// and `ptr` as VirtualPageNum as the key.
///
/// Iterate through the `PhysicalPageNum` of the terminal node associated
/// with this key until `len` fits in each page array, store it in an Vector,
/// and return it.
///
/// # Note
///
/// The kernel virtual address range for this buffer may not be contiguous.
///
/// # Parameters
/// - Token: Token in application address space.(the root node of `PhysPageNum`)
/// - ptr: Starting address of the buffer in its application address space, respectively.
/// - len: The length of the buffer in that application address space, respectively.
///        (note: The application virtual address range for this buffer is continuous).
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    // Write values to memory in page units.
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        // min((start + 1), (start + len))
        // Returns (start+1) if both are equal.
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}
