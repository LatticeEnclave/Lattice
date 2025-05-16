use core::{ops::DerefMut, ptr::NonNull};

use spin::Mutex;
use vm::{
    allocator::FrameAllocator,
    pm::{PhysAddr, PhysPageNum},
};

use crate::trampoline::Trampoline;

/// 管理整个Enclave的内存。
///
/// 一个Enclave中可能会有多个进程。
/// 外部的crate使用这个数据结构来创建和维护enclave相关的内存信息。
/// PhysMemMgr主要用于管理物理页，负责处理来自VirtMemManager的内存申请以及内存回收。
pub struct PhysMemMgr {
    head: Mutex<usize>,
    size: Mutex<usize>,
    trampoline: Trampoline,
}

impl PhysMemMgr {
    pub fn init() -> Self {
        todo!()
    }

    pub fn new(head: usize, size: usize, trampoline: usize) -> Self {
        Self {
            head: Mutex::new(head),
            size: Mutex::new(size),
            trampoline: Trampoline::create(trampoline),
        }
    }

    /// 增加需要管理的物理帧
    ///
    /// 考虑到物理帧可能不连续，因此一个一个添加，而不是按区域添加
    /// 该函数也会在物理帧被回收时调用
    pub fn add_frame(&self, ppn: PhysPageNum) {
        let addr = PhysAddr::from_ppn(ppn);
        // lock
        let mut lock = self.head.lock();
        // read locked head
        let val = lock.clone();
        // link new head
        unsafe { self.trampoline.writep(addr, val) };
        // update head
        *lock.deref_mut() = addr.into();
        // size added
        let mut size_lock = self.size.lock();
        *size_lock.deref_mut() += 0x1000;
    }

    /// 获取一个空闲帧
    pub fn get_free_frame(&self) -> Option<PhysPageNum> {
        let mut lock = self.head.lock();
        let val = lock.clone();
        let next: usize = unsafe { self.trampoline.readp(val) };
        *lock.deref_mut() = next;
        let mut size_lock = self.size.lock();
        *size_lock.deref_mut() -= 0x1000;

        Some(PhysPageNum::from_paddr(val))
    }

    /// get spare free list size
    pub fn get_spa_size(&self) -> usize {
        let size_lock = self.size.lock();
        size_lock.clone()
    }

    /// create a new pmm
    pub fn spawn_allocator(&self) -> RtFrameAlloc {
        RtFrameAlloc(NonNull::from(self))
    }

    pub fn clone_trampoline(&self) -> Trampoline {
        self.trampoline.clone()
    }

    pub unsafe fn readp(&self, addr: usize) -> usize {
        self.trampoline.readp::<usize, usize>(addr)
    }
    
    pub unsafe fn writep(&self, addr: usize, val: usize) {
        self.trampoline.writep::<usize, usize>(addr, val);
    }
}

#[derive(Clone)]
pub struct RtFrameAlloc(NonNull<PhysMemMgr>);

impl FrameAllocator for RtFrameAlloc {
    fn alloc(&self) -> Option<PhysPageNum> {
        unsafe { self.0.as_ref().get_free_frame() }
    }

    fn dealloc(&self, ppn: PhysPageNum) {
        unsafe { self.0.as_ref().add_frame(ppn) }
    }
}
