//! Implementation of [`MapArea`] and [`MemorySet`].

use crate::config::{MEMORY_END, PAGE_SIZE, USER_STACK_SIZE};
use crate::mm::address::StepByOne;

use super::address::VPNRange;
use super::page_table::PTEFlags;
use super::{frame_alloc, PhysPageNum, VirtAddr};
use super::{page_table::PageTable, FrameTracker, VirtPageNum};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

extern "C" {
    /// start text segment
    fn stext();
    /// end text segment
    fn etext();
    /// start read only data segment
    fn srodata();
    /// end read only data segment
    fn erodata();
    /// start data segment
    fn sdata();
    /// end data segment
    fn edata();
    /// start block starting symbol with kernel stack
    fn sbss_with_stack();
    /// end block starting symbol
    fn ebss();
    /// end kernel memory segment
    fn ekernel();
    /// start trampoline
    fn strampoline();
}

/// Expressing the address space.
///
/// The address space is a series of associated,
/// though not necessarily contiguous, logical segments,and in general,
/// the virtual memory space of this logical segment is bounded by the running program
/// (currently the running program is called a task, but in the future it will be called a process),
/// which means that It means that the direct access of the running program to code and data
/// is restricted to within the associated virtual address space.
/// This is why there are terms such as address space for task, address space for kernel, etc.
pub struct MemorySet {
    /// Maintain physical page frames for all nodes in the multi-level page table.
    page_table: PageTable,
    /// Virtual areas for each program.
    areas: Vec<MapArea>,
}

impl MemorySet {
    /// Creates a new empty address space.
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }

    /// Assume that no conflicts.
    ///
    /// # Note
    ///
    /// Ensure that two logical segments in the same address space cannot intersect.
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        todo!()
    }

    /// Generate kernel address space without kernel stacks.
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        println!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        println!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        println!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        memory_set
    }

    /// It parses the contents of the application's ELF file format,
    /// parses the data segment and generates the corresponding address space.
    ///
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        // ph_count: the number of all header
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            // Type::Load meaning that need to read program header by kernel.
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                // ph.mem_size: Memory size required for the application.
                //? The size for the number of zeros in the bss area is also calculated?
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                // push to address space.
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // plus guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // map TrapContext
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }
}

/// Contiguous virtual address (logical segment)
pub struct MapArea {
    /// Describes a contiguous section of virtual page number
    /// and indicates the location and length of logical segment in the address section.
    vpn_range: VPNRange,
    /// A key/value pair container that holds each virtual page in a logical segment
    /// and the `FrameTracker`, the physical page frame to which it is mapped.
    ///
    /// It is used to hold actual memory data, not as an intermediate node in a multi-level page table.
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    /// map type for memory set: identical or framed.
    map_type: MapType,
    /// A field that controls how the logical segment is accessed.
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                // To Physical page number == Virtual page number.
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                // Allocated physical page frame
                let frame = frame_alloc().unwrap();
                // ppn = Physical page number of the physical page frame
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }

    #[allow(unused)]
    /// It calls the unmap interface of `PageTable` to delete key/value pairs
    /// whose key is the passed virtual page number.
    ///
    /// However, when mapping with Framed,
    /// remove the physical page frame `FrameTracker`
    /// to which the virtual page is mapped from data_frames,
    /// so that the physical page frame can be immediately recycled for subsequent allocation.
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }

    /// Add mappings of the current logical segment to physical memory
    /// from the multilevel page table in the address space
    ///  to which the incoming logical segment belongs.
    ///
    /// These are implemented by iterating through all the virtual pages in the logical segment
    /// and inserting key/value pairs in the multi-level page table
    /// for each virtual page in turn.
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }

    /// Remove mappings of the current logical segment to physical memory
    /// from the multilevel page table in the address space
    ///  to which the incoming logical segment belongs.
    ///
    /// These are implemented by iterating through all the virtual pages in the logical segment
    /// and deleting key/value pairs in the multi-level page table
    /// for each virtual page in turn.
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }

    /// The data in the slice data is copied to a physical page frame
    /// where the kernel actually places the logical segment,
    /// allowing access to the data via that logical segment in the address space.
    ///
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        // Repeatedly copy data that did not make it into each page.
        loop {
            // The range of src is specified per page(4096) or within a page range(0<=(4096-1))
            //
            // # Example
            //
            // start = 4096
            // start + PAGE_SIZE(4096) = 8192
            // for example // start = 4096
            // 4096..8192 = 0 ~ 4096 = 1 page
            let src = &data[start..len.min(start + PAGE_SIZE)];
            // Change the PageTable passed in the argument
            let dst = &mut page_table
                // to a physical page number with a virtual page number.
                .translate(current_vpn)
                .unwrap()
                .ppn()
                // and get a mutable reference array of the physical memory area for that page.
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

bitflags! {
    /// A subset of the page table entry flags PTEFlags, leaving only the U/R/W/X flags.
    ///
    /// - The other flags are only concerned with details of the hardware address translation mechanism,
    ///   thus avoiding the introduction of incorrect flags.
    pub struct MapPermission: u8 {
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
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    /// A type that maps the same address as a virtual address to a physical address.
    ///
    /// VirtPageNum == PhysPageNum
    Identical,
    /// The actual allocation of memory and other resources for the application.
    ///
    /// The terminal node that does not point to the page table to the next.
    ///
    /// It represents the fact that for each virtual page,
    /// a new corresponding physical page frame is allocated,
    /// and the mapping between virtual and physical addresses is relatively random.
    Framed,
}