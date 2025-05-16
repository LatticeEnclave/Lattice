use core::mem::transmute;

use vm::{
    page_table::{PageTableEntry, PageTableReader},
    PageTableWriter,
};

use crate::trampoline::Trampoline;

#[derive(Clone)]
pub struct RtPtReader {
    trampoline: Trampoline,
}

impl RtPtReader {
    fn get_mut(&mut self, pt: usize, idx: usize) -> &'static mut PageTableEntry {
        let paddr = pt * 0x1000 + core::mem::size_of::<PageTableEntry>() * idx;
        let pte: &mut PageTableEntry = unsafe { transmute(paddr) };
        pte
    }

    fn get(&self, pt: usize, idx: usize) -> &'static PageTableEntry {
        let paddr = pt * 0x1000 + core::mem::size_of::<PageTableEntry>() * idx;
        let pte: &PageTableEntry = unsafe { transmute(paddr) };
        pte
    }
}

impl PageTableReader for RtPtReader {
    fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
        let pte_ref = self.get(pt, idx);
        unsafe { self.trampoline.read_ref(&pte_ref) }
    }

    fn next(&self, pt: usize, idx: usize) -> Option<usize> {
        let pte = self.read(pt, idx);
        if !pte.is_valid() || pte.is_leaf() {
            return None;
        }
        Some(pte.get_addr())
    }
}

#[derive(Clone)]
pub struct RtPtWriter {
    // for update page table entry
    // trampoline: Trampoline,
    reader: RtPtReader,
}

impl RtPtWriter {
    pub fn new(trampoline: Trampoline) -> Self {
        Self {
            reader: RtPtReader { trampoline },
        }
    }
}

impl PageTableReader for RtPtWriter {
    fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
        self.reader.read(pt, idx)
    }

    fn next(&self, pt: usize, idx: usize) -> Option<usize> {
        self.reader.next(pt, idx)
    }
}

impl PageTableWriter for RtPtWriter {
    fn write<M: vm::mm::MemModel>(&mut self, pt: usize, idx: usize, pte: PageTableEntry) {
        let pte_addr = self.reader.get_mut(pt, idx);
        unsafe {
            self.reader.trampoline.write_mut(pte_addr, pte);
        }
    }

    fn clean<M: vm::mm::MemModel>(&mut self, pt: usize, idx: usize) -> PageTableEntry {
        let old_pte = self.read(pt, idx);
        // wape the old pte
        self.write::<M>(pt, idx, PageTableEntry::from_bits(0));
        old_pte
    }
}