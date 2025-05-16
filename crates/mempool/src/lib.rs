#![no_std]

use core::ptr::NonNull;

use vm::align_up;

pub struct Mempool {
    start: NonNull<u8>,
    size: usize,
}

impl Mempool {
    pub fn new(array: &mut [u8]) -> Self {
        Self {
            start: unsafe { NonNull::new_unchecked(array.as_mut_ptr()) },
            size: array.len(),
        }
    }

    #[inline]
    pub fn alloc<T>(&mut self, val: T) -> Option<NonNull<T>> {
        let layout = core::alloc::Layout::new::<T>();
        let ptr = align_up!(self.start.addr().get(), layout.align());
        let offset = ptr - self.start.addr().get();
        if self.size - offset < layout.size() {
            return None;
        }
        self.size -= offset + layout.size();

        let ptr = NonNull::new(ptr as *mut T)?;
        unsafe { core::ptr::write(ptr.as_ptr(), val) };
        Some(ptr)
    }

    #[inline]
    pub fn get_start(&self) -> NonNull<u8> {
        self.start
    }
}

#[cfg(test)]
mod test {
    use crate::Mempool;

    struct A {
        pub a: usize,
        pub b: u32,
    }

    #[test]
    pub fn test_alloc() {
        let mut array = [0u8; 1024];

        let mut pool = Mempool::new(&mut array);

        let mut a = pool.alloc(1usize).unwrap();
        unsafe {
            *a.as_mut() = 1;
            assert_eq!(*a.as_ref(), 1);
        };
        let mut b = pool.alloc(A { a: 10, b: 1 }).unwrap();
        unsafe {
            // *a.as_mut() = 1;
            assert_eq!((*b.as_ref()).a, 10);
        };
    }
}
