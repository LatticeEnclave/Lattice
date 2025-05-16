use core::marker::PhantomData;

/// Allocate elements in a given array

const PTR_WIDTH: usize = core::mem::size_of::<usize>();

pub struct ArrayAllocator<T> {
    data_ptr: usize,
    len: usize,
    free_list: usize,
    free_size: usize,
    _phantom: PhantomData<T>,
}

impl<T> ArrayAllocator<T> {
    pub const unsafe fn uninit() -> Self {
        Self {
            data_ptr: 0,
            len: 0,
            free_list: 0,
            free_size: 0,
            _phantom: PhantomData,
        }
    }

    pub fn new(data: &mut [T]) -> Self {
        let free = data.as_mut_ptr() as usize;
        let mut allocator = Self {
            data_ptr: data.as_ptr() as usize,
            len: data.len(),
            free_list: free,
            free_size: 1,
            _phantom: PhantomData,
        };
        allocator.init(data);
        allocator
    }

    fn init(&mut self, data: &mut [T]) {
        unsafe {
            *(self.data_ptr as *mut usize) = 0;
        }
        for i in 1..self.len {
            unsafe {
                let elem = &*(&data[i] as *const T);
                self.add_free_element(elem);
            }
        }
    }

    pub fn add_free_element(&mut self, elem_ptr: &T) {
        let elem_ptr = elem_ptr as *const T as usize;
        let ptr = elem_ptr as *mut [u8; PTR_WIDTH];
        unsafe { set_next(ptr, self.free_list) }
        self.free_list = elem_ptr;
        self.free_size += 1;
    }

    /// Note: the element is not inited
    pub fn alloc_elem(&mut self) -> &'static mut T {
        let ptr = self.free_list as *const [u8; PTR_WIDTH];
        unsafe {
            let next = read_next(ptr);
            self.free_list = next;
            self.free_size -= 1;
            // set_next(ptr, addr)
            &mut *(ptr as *mut T)
        }
    }

    pub fn get_free_size(&self) -> usize {
        self.free_size
    }
}

unsafe fn set_next(ptr: *mut [u8; PTR_WIDTH], addr: usize) {
    *ptr = addr.to_be_bytes();
}

unsafe fn read_next(ptr: *const [u8; PTR_WIDTH]) -> usize {
    usize::from_be_bytes(*ptr)
}

#[cfg(test)]
mod test {
    use super::ArrayAllocator;

    #[test]
    pub fn test_array_alloc_init() {
        static mut ARRAY: [usize; 256] = [0; 256];
        unsafe {
            let ptr = ARRAY.as_mut_slice();
            let mut allocator = ArrayAllocator::new(ptr);
            assert_eq!(allocator.get_free_size(), 256);

            let elem = allocator.alloc_elem();

            assert_eq!(*elem, ARRAY[255]);
        }
    }

    #[test]
    pub fn test_array_alloc_alloc() {
        static mut ARRAY: [usize; 256] = [0; 256];

        unsafe {
            let ptr = ARRAY.as_mut_slice();
            assert_eq!(ptr.len(), 256);
            let mut allocator = ArrayAllocator::new(ptr);

            // let elem = allocator.alloc_elem();
            for _ in 0..256 {
                allocator.alloc_elem();
            }

            assert_eq!(allocator.free_list, 0);
            assert_eq!(allocator.get_free_size(), 0);
        }
    }

    #[test]
    pub fn test_array_alloc_recycle() {
        static mut ARRAY: [usize; 256] = [0; 256];

        unsafe {
            let ptr = ARRAY.as_mut_slice();
            let mut allocator = ArrayAllocator::new(ptr);

            let e1 = allocator.alloc_elem();
            *e1 = 1234;
            let e1_addr = e1 as *mut usize as usize;

            let e2 = allocator.alloc_elem();
            let e3 = allocator.alloc_elem();
            let e4 = allocator.alloc_elem();
            let e5 = allocator.alloc_elem();
            let e6 = allocator.alloc_elem();
            let e7 = allocator.alloc_elem();

            allocator.add_free_element(&e1);
            assert_eq!(allocator.free_list, e1_addr);

            let e1 = allocator.alloc_elem();
            assert_ne!(*e1, 1234);

            // assert_eq!(allocator.free, 0);
        }
    }
}
