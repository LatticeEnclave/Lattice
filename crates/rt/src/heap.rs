use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
};

use crate::scratch::Scratch;
use spin::Mutex;

pub struct Heap {
    pub inner: Mutex<buddy_system_allocator::Heap<32>>,
}

impl Heap {
    pub unsafe fn new(start: usize, size: usize) -> Self {
        let mut buddy = buddy_system_allocator::Heap::new();
        buddy.init(start, size);

        Self {
            inner: Mutex::new(buddy),
        }
    }

    /// Do real allocate
    pub fn alloc(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        self.inner.lock().alloc(layout).map_err(|_| AllocError)
    }

    pub fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.lock().dealloc(ptr, layout);
    }
}

pub struct LuHeapAllocator;

impl LuHeapAllocator {
    pub fn alloc_ptr(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        unsafe {
            Scratch::from_ssratch()
                .get_lu_kernel()
                .get_heap()
                .alloc(layout)
                .map_err(|_| AllocError)
        }
    }

    pub fn dealloc_ptr(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            Scratch::from_ssratch()
                .get_lu_kernel()
                .get_heap()
                .dealloc(ptr, layout)
        };
    }
}

unsafe impl Allocator for LuHeapAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.alloc_ptr(layout)
            .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.dealloc_ptr(ptr, layout)
    }
}

unsafe impl GlobalAlloc for LuHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_ptr(layout)
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_ptr(NonNull::new_unchecked(ptr), layout)
    }
}


pub struct LdHeapAllocator;

impl LdHeapAllocator {
    pub fn alloc_ptr(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        unsafe {
            Scratch::from_ssratch()
                .get_ld_kernel()
                .get_heap()
                .alloc(layout)
                .map_err(|_| AllocError)
        }
    }

    pub fn dealloc_ptr(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            Scratch::from_ssratch()
                .get_ld_kernel()
                .get_heap()
                .dealloc(ptr, layout)
        };
    }
}

unsafe impl Allocator for LdHeapAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.alloc_ptr(layout)
            .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.dealloc_ptr(ptr, layout)
    }
}

unsafe impl GlobalAlloc for LdHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_ptr(layout)
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_ptr(NonNull::new_unchecked(ptr), layout)
    }
}