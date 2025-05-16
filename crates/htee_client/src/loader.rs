use core::slice;
use std::io::Read;

use crate::{align_up, page::Page};

pub struct Loader {
    pub pages: &'static mut [Page],
    pub offset: usize,
}

impl Loader {
    pub fn new(pages: &'static mut [Page]) -> Self {
        Self { pages, offset: 0 }
    }

    pub fn get_start(&self) -> *const u8 {
        self.pages[0].as_ptr()
    }

    pub fn get_size(&self) -> usize {
        self.pages.len() * 0x1000
    }

    pub fn get_top(&self) -> *const u8 {
        unsafe { self.get_start().offset(self.offset as isize) }
    }

    pub fn alloc_meta_page(&mut self) -> &'static mut [u8] {
        self.alloc(0x1000, 0x1000)
    }

    pub fn mmap(&mut self, file: &str) -> Result<&'static [u8], std::io::Error> {
        let mut file = std::fs::File::open(file)?;
        let mut data = [0; 0x1000];

        let ptr = self.get_top();
        let mut total_size = 0;
        // let mut data = Vec::new();
        // file.read_to_end(&mut data).unwrap();

        loop {
            let len = file.read(&mut data)?;
            if len == 0 {
                break;
            }
            // file.read_exact(&mut data)
            total_size += len;

            let dst = self.alloc(data.len(), 0x1000);
            dst.copy_from_slice(&data);
        }

        Ok(unsafe { slice::from_raw_parts(ptr, total_size) })
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> &'static mut [u8] {
        self.align(align);
        if self.offset + size > self.get_size() {
            panic!("Memory size too small")
        }
        let array = unsafe { slice::from_raw_parts_mut((self.get_top()) as *mut u8, size) };

        for byte in array.iter_mut() {
            *byte = 0;
        }
        self.offset += size;
        array
    }

    pub fn alloc_tail(&mut self, size: usize) -> &'static mut [u8] {
        let num = (size + 0xfff) / 0x1000;
        if self.pages.len() < num {
            panic!("Memory size too small")
        }
        let len = self.pages.len();
        let remian_pages = unsafe { slice::from_raw_parts_mut(self.pages.as_mut_ptr(), len - num) };
        let pages = &mut self.pages[len - num..];
        self.pages = remian_pages;
        assert_eq!(pages.len(), num);
        let array = unsafe { slice::from_raw_parts_mut(pages.as_ptr() as *mut u8, size) };

        for byte in array.iter_mut() {
            *byte = 0;
        }

        array
    }

    pub fn alloc_vec(&mut self, ptr: *const u8, len: usize) -> *const u8 {
        let array = unsafe { slice::from_raw_parts(ptr as *mut u8, len) };
        self.alloc_array(array).as_ptr()
    }

    #[inline]
    pub fn align(&mut self, align: usize) -> &mut Self {
        self.offset = align_up!(self.offset, align);
        self
    }

    pub fn alloc_array<T>(&mut self, src: &[T]) -> &'static mut [T] {
        let size = src.len() * size_of::<T>();
        let src_byte = unsafe { slice::from_raw_parts(src.as_ptr() as *const u8, size) };
        let dst_byte = self.alloc(size, align_of::<T>());
        dst_byte.copy_from_slice(src_byte);
        let dst = unsafe { slice::from_raw_parts_mut(dst_byte.as_ptr() as *mut T, src.len()) };
        dst
    }

    pub fn get_remain_page(&mut self) -> &'static [u8] {
        self.align(0x1000);
        let size = self.get_size() - self.offset;
        unsafe { slice::from_raw_parts(self.get_top(), size) }
    }
}

#[cfg(test)]
mod test {
    #[test]
    pub fn test_string_to_bytes() {
        let s = "hello";
        let bytes = s.as_bytes();
        assert_eq!(bytes, &[104, 101, 108, 108, 111]);
        let s2 = std::str::from_utf8(bytes).unwrap();
        assert_eq!(s, s2);
    }
}
