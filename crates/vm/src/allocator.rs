use crate::pm::PhysPageNum;

pub trait FrameAllocator {
    fn alloc(&self) -> Option<PhysPageNum>;
    fn dealloc(&self, ppn: PhysPageNum);
}
