#![no_std]
use core::{mem::size_of, slice, usize};

#[derive(Debug)]
pub enum Error {
    NotEnoughStorage,
}

pub struct ArgRegs {
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
}

impl ArgRegs {
    fn empty() -> Self {
        Self {
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
        }
    }
}

/// Vstack is designed to be used as an arguments passing stack for the cross enclave call.
///
/// Typically, the enclave/host will allocate a Vstack instance in the untrusted memory,
/// and pass the address of the Vstack instance to the untrusted/enclave function.
/// Vstack is not designed to be used as multi-threading, and it is not thread-safe.
/// Thus, for every cross enclave call, the enclave/host should allocate a new Vstack instance.
#[repr(C)]
pub struct Vstack {
    pub regs: ArgRegs,
    size: usize,
    sp: usize,
}

impl Vstack {
    pub fn new(addr: usize, size: usize) -> &'static mut Self {
        let ptr = addr as *mut Self;
        let s = unsafe { &mut *(ptr) };
        *s = Vstack {
            regs: ArgRegs::empty(),
            size: size - size_of::<Self>(),
            sp: addr + size,
        };

        s
    }

    pub fn from_addr(addr: usize) -> &'static mut Self {
        let ptr = addr as *mut Self;
        unsafe { &mut *ptr }
    }

    pub fn store(&mut self, data: &[u8]) -> Result<usize, Error> {
        if self.remain_size() < data.len() {
            return Err(Error::NotEnoughStorage);
        }
        self.sp = self.sp - data.len();
        unsafe {
            let a = slice::from_raw_parts_mut(self.sp as *mut u8, data.len());
            a.copy_from_slice(data);
        }

        Ok(self.sp)
    }

    pub fn calc_offset(&self, addr: usize) -> Option<usize> {
        let base = self as *const Self as usize;
        if addr <= base {
            return None;
        }

        return Some(addr - base);
    }

    pub fn calc_addr(&self, offset: usize) -> usize {
        let base = self as *const Self as usize;
        return base + offset;
    }

    pub fn bp(&self) -> usize {
        (self as *const Self as usize) + size_of::<Self>()
    }

    pub fn sp(&self) -> usize {
        self.sp
    }

    pub fn remain_size(&self) -> usize {
        self.sp() - self.bp()
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
