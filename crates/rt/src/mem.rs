use core::ptr::NonNull;

use vm::page_table::{PageTableEntry, PageTableReader};
use vm::PageTableWriter;

use crate::pt::RtPtReader;
use crate::trampoline::Trampoline;
use crate::PhysMemMgr;

/// Virtual memory struct
pub struct Memory {}

pub struct RtPtWriter {
    // for alloc page table
    pmm: NonNull<PhysMemMgr>,
    trampoline: Trampoline,
}

impl RtPtWriter {
    pub fn new(pmm: &PhysMemMgr, trampoline: Trampoline) -> Self {
        Self {
            pmm: NonNull::from(pmm),
            trampoline,
        }
    }

    pub fn read_pte(&self, addr: usize, idx: usize) -> PageTableEntry {
        let pte = unsafe { self.trampoline.read_ref(RtPtReader.get(addr, idx)) };
        pte
    }

    pub fn write_pte(&mut self, addr: usize, idx: usize, pte: PageTableEntry) {
        let pte_addr = RtPtReader.get_mut(addr, idx);
        unsafe {
            self.trampoline.write_mut(pte_addr, pte);
        }
    }
}

impl PageTableReader for RtPtWriter {
    fn get_mut(&mut self, addr: usize, idx: usize) -> &'static mut PageTableEntry {
        RtPtReader.get_mut(addr, idx)
    }

    fn get(&self, addr: usize, idx: usize) -> &'static PageTableEntry {
        RtPtReader.get(addr, idx)
    }

    fn next(&self, addr: usize, idx: usize) -> Option<usize> {
        RtPtReader.next(addr, idx)
    }
}

impl PageTableWriter for RtPtWriter {
    fn write<M: vm::mm::MemModel>(
        &mut self,
        pt: usize,
        idx: usize,
        flags: vm::page_table::PTEFlags,
    ) {
        let pmm = unsafe { self.pmm.as_ref() };
        let new_ppn = pmm.get_free_frame().unwrap();
        let new_pte = PageTableEntry::new(new_ppn, flags);
        self.write_pte(pt, idx, new_pte);
    }

    fn clean<M: vm::mm::MemModel>(&mut self, pt: usize, idx: usize) {
        let pmm = unsafe { self.pmm.as_ref() };
        let old_pte = self.read_pte(pt, idx);
        let old_ppn = old_pte.get_ppn();
        pmm.add_frame(old_ppn);
        // wape the old pte
        self.write_pte(pt, idx, PageTableEntry::from_bits(0));
    }
}
