use core::cell::RefCell;
use htee_console::log;

use riscv::register::satp;
use vm::{
    Translate,
    allocator::FrameAllocator,
    page_table::BarePtReader,
    pm::{PhysAddr, PhysPageNum},
    vm::VirtAddr,
};

// pub struct SmPtWriter;

// impl PageTableReader for SmPtWriter {
//     fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
//         SmPtReader.read(pt, idx)
//     }

//     fn next(&self, addr: usize, idx: usize) -> Option<usize> {
//         SmPtReader.next(addr, idx)
//     }
// }

// impl PageTableWriter for SmPtWriter {
//     fn write<M: MemModel>(&mut self, pt: usize, idx: usize, pte: PageTableEntry) {
//         // update real page table
//         PageTable::from_addr(pt * 0x1000).get_array_mut()[idx] = pte;
//     }

//     fn clean<M: MemModel>(&mut self, pt: usize, idx: usize) -> PageTableEntry {
//         unimplemented!()
//     }
// }

// pub struct SmPtReader;

// impl SmPtReader {
//     fn get_mut(&mut self, pt: usize, idx: usize) -> &'static mut PageTableEntry {
//         &mut PageTable::from_addr(pt * 0x1000).get_array_mut()[idx]
//     }

//     fn get(&self, pt: usize, idx: usize) -> &'static PageTableEntry {
//         &PageTable::from_addr(pt * 0x1000).get_array_ref()[idx]
//     }
// }

// impl PageTableReader for SmPtReader {
//     fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
//         *self.get(pt, idx)
//     }

//     fn next(&self, pt: usize, idx: usize) -> Option<usize> {
//         let real_pte = self.read(pt, idx);
//         if !real_pte.is_valid() || real_pte.is_leaf() {
//             return None;
//         }

//         Some(real_pte.get_addr())
//     }
// }

/// A simple allocator
pub struct OneShotAllocator {
    pub root_ppn: usize,
    pub mode: satp::Mode,
    pub start: usize,
    // pub size: usize,
    pub end: usize,
}

impl OneShotAllocator {
    pub fn alloc_vpage(&mut self) -> Option<usize> {
        if self.start == self.end {
            None
        } else {
            let val = self.start;
            self.start += 0x1000;

            log::trace!("one shot allocator alloc vpage: {:#x}", val);
            Some(val)
        }
    }

    pub fn alloc_frame(&mut self) -> Option<PhysPageNum> {
        self.alloc_vpage()
            .map(|addr| VirtAddr::from(addr))
            .and_then(|vaddr| vaddr.translate(self.root_ppn, self.mode, &BarePtReader))
            .map(|paddr| PhysPageNum::from_paddr(paddr))
    }
}

pub struct OneShotAllocatorWrapper(pub RefCell<OneShotAllocator>);

impl FrameAllocator for OneShotAllocatorWrapper {
    fn alloc(&self) -> Option<PhysPageNum> {
        let ppn = self.0.borrow_mut().alloc_frame()?;
        let arr = unsafe { (PhysAddr::from_ppn(ppn).0 as *mut [u8; 512]).as_mut() }?;
        arr.iter_mut().for_each(|x| *x = 0);
        Some(ppn)
    }

    fn dealloc(&self, _: PhysPageNum) {
        unimplemented!()
    }
}
