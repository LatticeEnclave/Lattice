use core::{fmt::Display, iter::Step};

use bit_field::BitField;
use riscv::register::satp;

use crate::{
    align_up, allocator::FrameAllocator, consts::PAGE_SIZE, mm::*, page_table::{PTEFlags, PageTableEntry}, pm::PhysPageNum, translate::Translate, PageTableWriter
};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

impl Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl VirtAddr {
    pub const INVALID: Self = Self(0);

    pub fn from_vpn(vpn: impl Into<VirtPageNum>) -> Self {
        let vpn: VirtPageNum = vpn.into();
        Self(vpn.0 << 12)
    }

    pub fn add(self, offset: usize) -> Self {
        Self(self.0 + offset)
    }
}

impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl<T> From<*const T> for VirtAddr {
    fn from(value: *const T) -> Self {
        Self(value as usize)
    }
}

impl From<u64> for VirtAddr {
    fn from(v: u64) -> Self {
        Self(v as usize)
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(vpn: VirtPageNum) -> Self {
        Self::from_vpn(vpn)
    }
}

/// 虚拟内存页号
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct VirtPageNum(pub usize);

impl VirtPageNum {
    pub fn from_vaddr(vaddr: impl Into<VirtAddr>) -> Self {
        let vaddr: VirtAddr = vaddr.into();
        Self(vaddr.0.get_bits(12..))
    }

    pub fn add(self, count: usize) -> Self {
        Self(self.0 + count)
    }

    pub fn sub(self, count: usize) -> Self {
        Self(self.0 - count)
    }
}

impl From<usize> for VirtPageNum {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<VirtAddr> for VirtPageNum {
    fn from(value: VirtAddr) -> Self {
        Self::from_vaddr(value)
    }
}

impl Step for VirtPageNum {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        match end.0.checked_sub(start.0) {
            Some(step) => (step, Some(step)),
            None => (0, None),
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_add(count).map(|v| VirtPageNum(v))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_sub(count).map(|v| VirtPageNum(v))
    }
}

/// 连续的虚拟内存地址
#[derive(Clone, Copy)]
pub struct VirtMemArea {
    pub start: usize,
    pub size: usize,
    pub flags: PTEFlags,
    pub satp: satp::Satp,
}

impl Default for VirtMemArea {
    fn default() -> Self {
        Self {
            start: 0,
            size: 0,
            flags: PTEFlags::empty(),
            satp: satp::read(),
        }
    }
}

impl VirtMemArea {
    #[inline]
    pub fn start(mut self, start: impl Into<VirtAddr>) -> Self {
        let start: VirtAddr = start.into();
        self.start = start.0;
        self
    }

    pub fn size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn satp(mut self, satp: satp::Satp) -> Self {
        self.satp = satp;
        self
    }

    pub fn flags(mut self, flags: PTEFlags) -> Self {
        self.flags = flags;
        self
    }

    #[inline]
    pub fn iter_vpn(&self) -> impl Iterator<Item = VirtPageNum> {
        (self.start..(self.start + align_up!(self.size, PAGE_SIZE)))
            .step_by(PAGE_SIZE)
            .map(|addr| VirtPageNum::from_vaddr(addr))
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

pub type Sv39VmMgr<W, A> = VirtMemMgr<W, A, SV39>;
pub type Sv48VmMgr<W, A> = VirtMemMgr<W, A, SV48>;

/// 管理每个进程的虚拟内存
///
/// 负责建立页表映射。
#[derive(Clone)]
pub struct VirtMemMgr<W: PageTableWriter, A: FrameAllocator, M: MemModel> {
    pub root_ppn: PhysPageNum,
    pub asid: usize,
    pub writer: W,
    pub frame_allocator: A,
    pub mm: M,
    // _phantom: PhantomData<M>,
}

/// 映射一个虚拟页到物理页
impl<W: PageTableWriter, A: FrameAllocator, M: MemModel> VirtMemMgr<W, A, M> {
    pub fn new(root_ppn: PhysPageNum, writer: W, frame_allocator: A, asid: usize, mm: M) -> Self {
        VirtMemMgr {
            root_ppn,
            asid,
            writer,
            frame_allocator,
            mm, // _phantom: PhantomData::default(),
        }
    }

    pub fn from_reg(writer: W, frame_allocator: A) -> Self {
        let satp = satp::read();
        VirtMemMgr {
            root_ppn: PhysPageNum::from(satp.ppn()),
            asid: satp.asid(),
            writer,
            frame_allocator,
            mm: M::default(),
        }
    }

    pub fn map_frame(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let mut pt = self.root_ppn.0;
        let idxs = M::split_vpn(vpn);
        let mut level = M::LEVEL;
        while level != 1 {
            level = level - 1;
            let pte = self.writer.read(pt, idxs[level]);
            if !pte.is_valid() {
                // 如果中间节点的页表项不存在,分配一个新的页表
                let new_pt = self.frame_allocator.alloc().unwrap();
                let new_pte = PageTableEntry::new(new_pt, PTEFlags::V);
                self.writer.write::<M>(pt, idxs[level], new_pte);
                pt = new_pt.0; // 更新 pt 变量
            } else if pte.is_valid() && !pte.is_leaf() {
                // 更新到下一级
                pt = pte.get_ppn().0;
            } else {
                // 如果中间节点的页表项已经存在且是叶子节点,说明出现了映射冲突
                panic!("Mapping conflict: PTE is already a leaf node");
            }
        }

        let pte = self.writer.read(pt, idxs[0]);
        if !pte.is_valid() {
            let new_pte = PageTableEntry::new(ppn, flags);
            self.writer.write::<M>(pt, idxs[0], new_pte);
        } else {
            panic!("Mapping conflict: PTE is already a leaf node");
        }
    }

    #[inline]
    pub fn alloc_new_page(&mut self, vpn: VirtPageNum, flags: PTEFlags) -> Option<PhysPageNum> {
        let ppn = self.frame_allocator.alloc()?;
        self.map_frame(vpn, ppn, flags);
        Some(ppn)
    }

    pub fn alloc_vma(&mut self, mut vma: VirtMemArea) -> Option<VirtMemArea> {
        for vpn in vma.iter_vpn() {
            let ppn = self.frame_allocator.alloc()?;
            self.map_frame(vpn, ppn, vma.flags);
        }
        vma.satp = satp::Satp::from_bits(self.gen_satp());
        Some(vma)
    }

    /// 取消一个物理页的映射，但不回收
    pub fn unmap_frame(&mut self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        let mut pt = self.root_ppn.0;
        let idxs = M::split_vpn(vpn);
        let mut level = M::LEVEL;
        while level != 1 {
            level = level - 1;
            let pte = self.writer.read(pt, idxs[level]);
            if pte.is_valid() && !pte.is_leaf() {
                pt = pte.get_ppn().0;
            } else {
                return None;
            }
        }
        let pte = self.writer.read(pt, idxs[0]);
        if pte.is_valid() && pte.is_leaf() {
            let pte = self.writer.clean::<M>(pt, idxs[0]);
            Some(pte)
        } else {
            None
        }
    }

    pub fn get_pte(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        let mut pt = self.root_ppn.0;
        let idxs = M::split_vpn(vpn);
        let mut level = M::LEVEL;
        while level != 1 {
            level = level - 1;
            let pte = self.writer.read(pt, idxs[level]);
            if pte.is_valid() && !pte.is_leaf() {
                pt = pte.get_ppn().0;
            } else {
                return None;
            }
        }
        let pte = self.writer.read(pt, idxs[0]);
        if pte.is_valid() && pte.is_leaf() {
            Some(pte)
        } else {
            None
        }
    }

    /// 取消一个物理页的映射，并且回收
    pub fn dealloc_frame(&mut self, vpn: VirtPageNum) -> bool {
        let mut pt = self.root_ppn.0;
        let idxs = M::split_vpn(vpn);
        let mut level = M::LEVEL;
        while level != 1 {
            level = level - 1;
            let pte = self.writer.read(pt, idxs[level]);
            if pte.is_valid() && !pte.is_leaf() {
                pt = pte.get_ppn().0;
            } else {
                return false;
            }
        }
        let pte = self.writer.read(pt, idxs[0]);
        if pte.is_valid() && pte.is_leaf() {
            let pte = self.writer.clean::<M>(pt, idxs[0]);
            self.frame_allocator.dealloc(pte.get_ppn());
            true
        } else {
            false
        }
    }

    /// modify a physical page's mapping attributes
    pub fn remap_frame(&mut self, vpn: VirtPageNum, flags: PTEFlags) -> bool {
        let mut pt = self.root_ppn.0;
        let idxs = M::split_vpn(vpn);
        let mut level = M::LEVEL;
        while level != 1 {
            level = level - 1;
            let pte = self.writer.read(pt, idxs[level]);
            if pte.is_valid() && !pte.is_leaf() {
                pt = pte.get_ppn().0;
            } else {
                return false;
            }
        }
        let pte = self.writer.read(pt, idxs[0]);
        if pte.is_valid() && pte.is_leaf() {
            let new_pte = PageTableEntry::new(pte.get_ppn(), flags);
            self.writer.write::<M>(pt, idxs[0], new_pte);
            true
        } else {
            false
        }
    }

    /// 取消映射一片连续的虚拟地址。
    pub fn unmap_vma(&mut self, start: usize, size: usize) -> bool {
        for vaddr in (start..(start + size)).step_by(PAGE_SIZE) {
            let ret = self.unmap_frame(VirtPageNum::from_vaddr(vaddr));
            if ret.is_none() {
                return false;
            }
        }
        return true;
    }

    /// 回收一片连续的虚拟地址。
    pub fn dealloc_vma(&mut self, start: usize, size: usize) -> bool {
        for vaddr in (start..(start + size)).step_by(PAGE_SIZE) {
            // clear the page
            unsafe {
                core::ptr::write_bytes(vaddr as *mut u8, 0, PAGE_SIZE);
            }
            let ret = self.dealloc_frame(VirtPageNum::from_vaddr(vaddr));
            if !ret {
                return false;
            }
        }
        return true;
    }

    /// Remap a continue virtual memory area.
    ///
    /// Notes: User must ensure `src.abs_diff(dst) <= size`
    pub fn remap_vma(&mut self, start: usize, size: usize, flags: PTEFlags) -> bool {
        for vaddr in (start..(start + size)).step_by(PAGE_SIZE) {
            let ret = self.remap_frame(VirtPageNum::from_vaddr(vaddr), flags);
            if !ret {
                return false;
            }
        }
        return true;
    }

    /// 映射一片连续的物理内存到虚拟内存上
    pub fn map_frames(
        &mut self,
        vpn: VirtPageNum,
        ppn: PhysPageNum,
        num: usize,
        flags: PTEFlags,
    ) -> VirtPageNum {
        for i in 0..num {
            self.map_frame(vpn.add(i), ppn.add(i), flags);
        }

        vpn.add(num)
    }

    pub fn gen_satp(&self) -> usize {
        (M::ID) << 60 | (self.asid << 44) | self.root_ppn.0
    }

    pub fn update_satp(&self) -> satp::Satp {
        let old = satp::read();
        satp::write(self.gen_satp());
        old
        // let satp = self.gen_satp();
        // self.writer.write::<M>(0, 0, PageTableEntry::new(self.root_ppn, PTEFlags::V));
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PhysPageNum> {
        vpn.trans_2_pm(self.root_ppn, &self.writer, self.mm)
            .map(|paddr| paddr.get_ppn())
    }
}
