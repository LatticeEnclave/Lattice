use bit_field::BitField;

use crate::consts::PAGE_SIZE;

pub struct Frame(&'static mut [u8; 0x1000]);

impl From<PhysAddr> for Frame {
    fn from(value: PhysAddr) -> Self {
        unsafe { Frame(&mut *(value.0 as *mut [u8; 0x1000])) }
    }
}

impl From<PhysPageNum> for Frame {
    fn from(value: PhysPageNum) -> Self {
        unsafe { Frame(&mut *((value.0 * 0x1000) as *mut [u8; 0x1000])) }
    }
}

impl Frame {
    pub fn clone_from(&mut self, src: &Frame) {
        self.0.clone_from_slice(src.0);
    }
}

pub struct PhsyMemPageList {
    next: Option<usize>,
}

impl PhsyMemPageList {
    pub fn iter(&self) -> impl Iterator<Item = PhysAddr> {
        let mut next = Some(self as *const PhsyMemPageList as usize);
        core::iter::from_fn(move || {
            let ret = next?;
            let list = unsafe { &*(ret as *const PhsyMemPageList) };
            next = list.next;

            Some(PhysAddr(ret))
        })
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

impl Into<usize> for PhysAddr {
    fn into(self) -> usize {
        self.0
    }
}

impl From<usize> for PhysAddr {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PhysAddr {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn ppn(mut self, ppn: PhysPageNum) -> Self {
        self.0.set_bits(12..=55, ppn.0);
        self
    }

    pub fn from_ppn(ppn: PhysPageNum) -> Self {
        Self::empty().ppn(ppn)
    }

    pub fn from_addr(addr: usize) -> Self {
        Self(addr)
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.0.set_bits(0..=11, offset);
        self
    }

    pub fn get_ppn(&self) -> PhysPageNum {
        self.floor()
    }

    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }

    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
}

/// 物理内存页号
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

impl From<usize> for PhysPageNum {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(value: PhysAddr) -> Self {
        Self::from_paddr(value)
    }
}

impl PhysPageNum {
    pub const INVALID: Self = Self(0);

    pub fn from_reg() -> Self {
        use riscv::register::satp;

        Self(satp::read().ppn())
    }

    pub fn from_paddr(paddr: impl Into<PhysAddr>) -> Self {
        Self(paddr.into().0 >> 12)
    }

    pub fn add(self, count: usize) -> Self {
        Self(self.0 + count)
    }
}
