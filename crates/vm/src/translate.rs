use riscv::register::satp;

use crate::{
    mm::*,
    page_table::PageTableEntry,
    pm::{PhysAddr, PhysPageNum},
    vm::{VirtAddr, VirtPageNum},
    PageTableReader,
};

pub trait Translate {
    type Target;

    fn trans_2_pm<'a, M: MemModel, R: PageTableReader>(
        self,
        pt: PhysPageNum,
        reader: &'a R,
        mm: M,
    ) -> Option<Self::Target>;

    fn translate<R: PageTableReader>(
        self,
        ppn: impl Into<PhysPageNum>,
        mode: satp::Mode,
        reader: &R,
    ) -> Option<PhysAddr>;
}

impl<T: Into<VirtAddr>> Translate for T {
    type Target = PhysAddr;

    fn trans_2_pm<'a, M: MemModel, R: PageTableReader>(
        self,
        ppn: PhysPageNum,
        reader: &'a R,
        mm: M,
    ) -> Option<PhysAddr> {
        let vaddr = self.into();
        let translator = VAddrTranslator::new(M::get_vpn(vaddr), ppn, reader, mm);

        let paddr = translator
            .translate_with_level()
            .map(|(ppn, level)| M::concat_paddr(ppn, M::get_offset(vaddr, level)));
        paddr
    }

    fn translate<R: PageTableReader>(
        self,
        ppn: impl Into<PhysPageNum>,
        mode: satp::Mode,
        reader: &R,
    ) -> Option<PhysAddr> {
        let vaddr: VirtAddr = self.into();
        match mode {
            satp::Mode::Bare => Some(PhysAddr(vaddr.0)),
            satp::Mode::Sv39 => Translate::trans_2_pm(vaddr, ppn.into(), reader, SV39),
            satp::Mode::Sv48 => Translate::trans_2_pm(vaddr, ppn.into(), reader, SV48),
            _ => None,
        }
    }
}

pub struct VAddrTranslator<'a, R: PageTableReader, M: MemModel> {
    pub ppn: PhysPageNum,
    pub vpn: VirtPageNum,
    pub reader: &'a R,
    pub _mm: M,
}

impl<'a, R: PageTableReader, M: MemModel> VAddrTranslator<'a, R, M> {
    #[inline]
    pub fn new(vpn: VirtPageNum, ppn: PhysPageNum, reader: &'a R, mm: M) -> Self {
        Self {
            ppn,
            vpn,
            reader,
            _mm: mm,
        }
    }

    #[inline]
    pub fn iter_pte(&self) -> PTEIterator<'a, R> {
        PTEIterator {
            ppn: self.ppn,
            vpns: M::split_vpn(self.vpn),
            level: M::LEVEL,
            is_leaf: false,
            is_fault: false,
            reader: &self.reader,
        }
    }

    #[inline]
    pub fn translate(self) -> Option<PhysPageNum> {
        self.iter_pte().get_leaf().map(|pte| pte.get_ppn())
    }

    pub fn translate_with_level(self) -> Option<(PhysPageNum, usize)> {
        let mut iter = self.iter_pte();
        let pte = iter.get_leaf()?;
        let ppn = pte.get_ppn();
        let level = iter.get_level();
        Some((ppn, level))
    }
}

pub struct PTEIterator<'a, R> {
    pub ppn: PhysPageNum,
    pub vpns: [usize; 5],
    pub reader: &'a R,
    pub level: usize,
    pub is_leaf: bool,
    pub is_fault: bool,
}

impl<'a, R: PageTableReader> PTEIterator<'a, R> {
    pub fn get_leaf(&mut self) -> Option<PageTableEntry> {
        let mut leaf = None;
        while let Some(ppn) = self.next() {
            leaf = Some(ppn);
        }

        if self.is_fault {
            return None;
        }

        leaf
    }

    #[inline]
    pub fn get_level(&mut self) -> usize {
        self.level
    }
}

impl<'a, R: PageTableReader> Iterator for PTEIterator<'a, R> {
    type Item = PageTableEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_leaf || self.is_fault {
            return None;
        }
        if self.level == 0 {
            return None;
        }
        self.level -= 1;
        let pte = self.reader.read(self.ppn.0, self.vpns[self.level] as usize);
        if !pte.is_valid() {
            self.is_fault = true;
            return None;
        }
        // is leaf pte
        if pte.is_leaf() {
            self.is_leaf = true;
        }
        self.ppn = pte.get_ppn();

        Some(pte)
    }
}
