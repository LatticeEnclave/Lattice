#![no_std]
mod alloc;
mod map;

use core::{slice, sync::atomic::AtomicBool};
use map::PoolBitMap;

pub const PER_SIZE: usize = 4096;

#[repr(C)]
pub struct MemPool {
    pub is_using: AtomicBool,
    pub start: *mut [u8; PER_SIZE],
    pub size: usize,
    pub bitmap: PoolBitMap,
}

impl MemPool {
    pub fn new(addr: usize, size: usize) -> &'static mut Self {
        let pool = unsafe { &mut *(addr as *mut Self) };

        pool.size = size - 1;
        pool.start = addr as *mut [u8; PER_SIZE];
        pool.is_using = AtomicBool::new(false);

        assert!(size % 8 == 0);
        let bits_ptr = (addr + size_of::<Self>()) as *mut u8;
        let bits_len = size / 8;
        let bits = unsafe { slice::from_raw_parts_mut(bits_ptr, bits_len) };
        for i in 0..bits_len {
            bits[i] = 0;
        }
        pool.bitmap = PoolBitMap {
            data: bits_ptr,
            len: bits_len,
        };

        // set the first bit to 1, cause it is used by the pool itself.
        pool.bitmap.set(0);

        pool
    }

    pub unsafe fn alloc(&mut self, size: usize) -> *const u8 {
        self.is_using
            .store(true, core::sync::atomic::Ordering::Relaxed);

        let num = (size + PER_SIZE - 1) / PER_SIZE;
        let mut ptr = core::ptr::null();
        if let Some(idx) = alloc::Allocator::alloc(num, &mut self.bitmap) {
            ptr = unsafe { self.start.add(idx) } as *const u8;
        }

        self.is_using
            .store(false, core::sync::atomic::Ordering::Relaxed);

        ptr
    }

    pub unsafe fn dealloc(&mut self, ptr: *const u8, size: usize) {
        self.is_using
            .store(true, core::sync::atomic::Ordering::Relaxed);

        let idx = (ptr as usize - self.start as usize) / PER_SIZE;
        let num = (size + PER_SIZE - 1) / PER_SIZE;
        for i in idx..idx + num {
            self.bitmap.clear(i);
        }

        self.is_using
            .store(false, core::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test {
    use crate::MemPool;

    #[test]
    pub fn test_create() {
        let mut data = [0u8; 3 * 8 * 4096];

        let _ = MemPool::new(data.as_mut_ptr() as usize, data.len());
    }

    #[test]
    pub fn test_alloc() {
        let mut data = [0u8; 3 * 8 * 4096];

        let pool = MemPool::new(data.as_mut_ptr() as usize, data.len());
        let ptr = unsafe { pool.alloc(4096) };
        assert!(!ptr.is_null());
        assert_eq!(ptr, (&data[4096]) as *const u8);
        assert!(pool.bitmap.test(1));

        let ptr = unsafe { pool.alloc(2 * 4096 + 1) };
        assert!(!ptr.is_null());
        assert!(pool.bitmap.test(4));
    }

    #[test]
    pub fn test_dealloc() {
        let mut data = [0u8; 3 * 8 * 4096];

        let pool = MemPool::new(data.as_mut_ptr() as usize, data.len());
        let ptr = unsafe { pool.alloc(4096) };
        unsafe {
            pool.dealloc(ptr, 4096);
        }
        assert!(!pool.bitmap.test(1));
    }
}
