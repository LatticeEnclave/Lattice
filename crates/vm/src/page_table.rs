use core::marker::PhantomData;

use bit_field::BitField;
use bitflags::bitflags;

use crate::{
    allocator::FrameAllocator,
    mm::{MemModel, SV48},
    pm::{PhysAddr, PhysPageNum},
};

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

impl PTEFlags {
    /// Non-U-mode RWX
    pub fn rwx() -> Self {
        PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::X
    }

    /// Non-U-mode RX
    pub fn rx() -> Self {
        PTEFlags::V | PTEFlags::R | PTEFlags::X
    }

    /// Non-U-mode RW
    pub fn rw() -> Self {
        PTEFlags::V | PTEFlags::R | PTEFlags::W
    }

    /// U-mode RWX
    pub fn urwx() -> Self {
        PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::X | PTEFlags::U
    }

    /// U-mode RW
    pub fn urw() -> Self {
        PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U
    }

    pub fn dirty(self) -> Self {
        self | PTEFlags::D
    }

    pub fn accessed(self) -> Self {
        self | PTEFlags::A
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: impl Into<PhysPageNum>, flags: PTEFlags) -> Self {
        let ppn: PhysPageNum = ppn.into();

        Self {
            bits: ppn.0 << 10 | flags.bits() as usize,
        }
    }

    pub fn get_ppn(&self) -> PhysPageNum {
        PhysPageNum(self.bits.get_bits(10..=53))
    }

    pub fn get_flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits.get_bits(..=7) as u8)
    }

    pub fn get_addr(&self) -> usize {
        self.bits.get_bits(10..=53) * 0x1000
    }

    pub fn is_x(&self) -> bool {
        self.bits.get_bit(3)
    }

    pub fn is_w(&self) -> bool {
        self.bits.get_bit(2)
    }

    pub fn is_r(&self) -> bool {
        self.bits.get_bit(1)
    }

    pub fn is_v(&self) -> bool {
        self.bits.get_bit(0)
    }

    pub fn is_valid(&self) -> bool {
        if !self.is_v() {
            false
        } else if !self.is_r() && self.is_w() {
            false
        } else {
            true
        }
    }

    pub fn is_leaf(&self) -> bool {
        if self.is_r() || self.is_x() {
            true
        } else {
            false
        }
    }

    pub fn from_bits(bits: usize) -> Self {
        Self { bits }
    }
}

impl From<usize> for PageTableEntry {
    fn from(value: usize) -> Self {
        Self::from_bits(value)
    }
}

impl Into<usize> for PageTableEntry {
    fn into(self) -> usize {
        self.bits
    }
}

pub type PageTable = PageTable64;

pub struct PageTable64([PageTableEntry; 512]);

impl PageTable64 {
    pub fn from_reg_mut() -> &'static mut Self {
        use riscv::register::satp;

        let satp = satp::read();
        let ppn = satp.ppn();
        let mode = satp.mode();

        let page_size = match mode {
            satp::Mode::Sv48 => SV48::PAGE_SIZE,
            _ => 0x1000,
        };

        Self::from_ppn(PhysPageNum(ppn), page_size)
    }

    pub fn from_addr(addr: usize) -> &'static mut Self {
        unsafe { &mut *(addr as *mut Self) }
    }

    pub fn from_ppn(ppn: PhysPageNum, page_size: usize) -> &'static mut Self {
        Self::from_addr(ppn.0 * page_size)
    }

    pub fn get_pte(&self, offset: usize) -> PageTableEntry {
        self.0[offset]
    }

    pub fn get_array_ref(&self) -> &[PageTableEntry; 512] {
        &self.0
    }

    pub fn get_array_mut(&mut self) -> &mut [PageTableEntry; 512] {
        &mut self.0
    }

    pub fn get_addr(&self) -> PhysAddr {
        PhysAddr(self as *const PageTable64 as usize)
    }
}

/// Manager page table
pub struct PageTableBuilder<W, M, A>
where
    W: PageTableWriter,
    M: MemModel,
    A: FrameAllocator,
{
    root: usize,
    // a custome page table writer
    writer: W,
    frame_allocator: A,
    _phantom: PhantomData<M>,
}

impl<W, M, A> PageTableBuilder<W, M, A>
where
    W: PageTableWriter,
    M: MemModel,
    A: FrameAllocator,
{
    pub fn new(root: usize, writer: W, frame_allocator: A, _: M) -> Self {
        Self {
            root,
            writer,
            frame_allocator,
            _phantom: PhantomData::default(),
        }
    }

    // pub fn find_or_create_pte(
    //     &mut self,
    //     vpn: impl Into<VirtPageNum>,
    // ) -> Option<PageTableEntry> {
    //     let vpns = M::split_vpn(vpn.into());

    //     let mut pt = self.root;
    //     let mut res = None;

    //     for (i, idx) in vpns.into_iter().enumerate() {
    //         let pte = self.writer.read(self.root, idx);

    //         if i == M::LEVEL - 1 {
    //             res = Some(pte);
    //             break;
    //         }

    //         if !pte.is_valid() {
    //             let ppn = self.frame_allocator.alloc().unwrap();
    //             self.writer
    //                 .write::<M>(pt.clone(), idx, PageTableEntry::new(ppn, PTEFlags::V));
    //         }

    //         pt = self.writer.next(pt, idx).unwrap();
    //     }

    //     res
    // }

    // pub fn find_pte(&mut self, vpn: impl Into<VirtPageNum>) -> Option<&mut PageTableEntry> {
    //     let vpns = M::split_vpn(vpn.into());

    //     let mut pt = self.root;
    //     let mut res = None;

    //     for (i, idx) in vpns.into_iter().enumerate() {
    //         let pte = self.writer.get_mut(self.root, idx);

    //         if i == M::LEVEL - 1 {
    //             res = Some(pte);
    //             break;
    //         }

    //         if !pte.is_valid() {
    //             res = None;
    //             break;
    //         }

    //         pt = self.writer.next(pt, idx).unwrap();
    //     }

    //     res
    // }

    // pub fn map(
    //     &mut self,
    //     vpn: impl Into<VirtPageNum>,
    //     ppn: impl Into<PhysPageNum>,
    //     flags: PTEFlags,
    // ) {
    //     let pte = self.find_or_create_pte(vpn.into()).unwrap();

    //     *pte = PageTableEntry::new(ppn.into(), flags | PTEFlags::V)
    // }

    // take frame_allocator
    pub fn finish(self) -> A {
        self.frame_allocator
    }
}

pub trait PageTableReader
where
    Self: Sized,
{
    // fn get_mut(&mut self, pt: usize, idx: usize) -> &'static mut PageTableEntry;

    // fn get(&self, pt: usize, idx: usize) -> &'static PageTableEntry;

    /// @pt: the ppn of page table
    fn read(&self, pt: usize, idx: usize) -> PageTableEntry;

    /// Get next level page table walker.
    /// Return None if pte is invalid, or
    fn next(&self, pt: usize, idx: usize) -> Option<usize>;
}

/// The way to change the page table.
///
/// For SM, it can directly access page tables as they run on the physical address space.
/// But for RT, it need to use a trampoline that switch the address space.
pub trait PageTableWriter: PageTableReader {
    /// alloc a free page and write the pte page table with flags
    fn write<M: MemModel>(&mut self, pt: usize, idx: usize, pte: PageTableEntry);

    /// dealloc the frame of the pte at given idx
    fn clean<M: MemModel>(&mut self, pt: usize, idx: usize) -> PageTableEntry;
}

pub struct BarePtWriter;

impl PageTableReader for BarePtWriter {
    fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
        BarePtReader.read(pt, idx)
    }

    fn next(&self, addr: usize, idx: usize) -> Option<usize> {
        BarePtReader.next(addr, idx)
    }
}

impl PageTableWriter for BarePtWriter {
    fn write<M: MemModel>(&mut self, pt: usize, idx: usize, pte: PageTableEntry) {
        // update real page table
        PageTable::from_addr(pt * 0x1000).get_array_mut()[idx] = pte;
    }

    fn clean<M: MemModel>(&mut self, pt: usize, idx: usize) -> PageTableEntry {
        unimplemented!()
    }
}

pub struct BarePtReader;

impl BarePtReader {
    #[allow(unused)]
    fn get_mut(&mut self, pt: usize, idx: usize) -> &'static mut PageTableEntry {
        &mut PageTable::from_addr(pt * 0x1000).get_array_mut()[idx]
    }

    fn get(&self, pt: usize, idx: usize) -> &'static PageTableEntry {
        &PageTable::from_addr(pt * 0x1000).get_array_ref()[idx]
    }
}

impl PageTableReader for BarePtReader {
    fn read(&self, pt: usize, idx: usize) -> PageTableEntry {
        *self.get(pt, idx)
    }

    fn next(&self, pt: usize, idx: usize) -> Option<usize> {
        let real_pte = self.read(pt, idx);
        if !real_pte.is_valid() || real_pte.is_leaf() {
            return None;
        }

        Some(real_pte.get_addr())
    }
}
