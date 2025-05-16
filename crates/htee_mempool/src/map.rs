/// Each bit correspond to a single memory trunk.
pub struct PoolBitMap {
    /// Pointer to the bitmap data.
    pub data: *mut u8,
    /// Must be a multiple of 8.
    pub len: usize,
}

impl PoolBitMap {
    pub fn set(&mut self, index: usize) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        let byte = unsafe { self.data.add(byte_index) };
        unsafe {
            *byte |= 1 << bit_index;
        }
    }

    pub fn clear(&mut self, index: usize) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        let byte = unsafe { self.data.add(byte_index) };
        unsafe {
            *byte &= !(1 << bit_index);
        }
    }

    pub fn test(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;
        let byte = unsafe { self.data.add(byte_index) };
        unsafe { *byte & (1 << bit_index) != 0 }
    }
}
