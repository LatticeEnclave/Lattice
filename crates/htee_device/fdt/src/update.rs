use crate::parsing::{BigEndianU32, BigEndianU64};

pub struct FdtUpdater<'a> {
    bytes: &'a mut [u8],
    idx: usize,
}

impl<'a> FdtUpdater<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, idx: 0 }
    }

    pub fn skip(&mut self, n_bytes: usize) {
        self.idx += n_bytes;
    }

    pub fn u32(&mut self) -> Option<BigEndianU32> {
        let ret = BigEndianU32::from_bytes(&self.bytes[self.idx..])?;
        self.skip(4);

        Some(ret)
    }

    pub fn u64(&mut self) -> Option<BigEndianU64> {
        let ret = BigEndianU64::from_bytes(&self.bytes[self.idx..])?;
        self.skip(8);

        Some(ret)
    }

    pub fn remaining(self) -> &'a mut [u8] {
        &mut self.bytes[self.idx..]
    }

    pub fn peek_u32(&self) -> Option<BigEndianU32> {
        let ret = BigEndianU32::from_bytes(&self.bytes[self.idx..])?;

        Some(ret)
    }

    pub fn peek_u64(&self) -> Option<BigEndianU64> {
        let ret = BigEndianU64::from_bytes(&self.bytes[self.idx..])?;

        Some(ret)
    }

    pub fn update_u32(&mut self, value: u32) -> Option<BigEndianU32> {
        let bytes = value.to_be_bytes();
        let old_val = self.peek_u32();
        if self.bytes[self.idx..].len() < bytes.len() {
            return None;
        }
        for i in 0..4 {
            self.bytes[self.idx + i] = bytes[i];
        }

        old_val
    }

    pub fn update_u64(&mut self, value: u64) -> Option<BigEndianU64> {
        let bytes = value.to_be_bytes();
        let old_val = self.peek_u64();
        if self.bytes[self.idx..].len() < bytes.len() {
            return None;
        }
        for i in 0..8 {
            self.bytes[self.idx + i] = bytes[i];
        }

        old_val
    }
}
