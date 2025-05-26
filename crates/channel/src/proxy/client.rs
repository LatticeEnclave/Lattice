use core::arch::asm;
use vstack::ArgRegs;

pub struct Client {
    raw: &'static mut [u8],
    regs: &'static mut ArgRegs,
}

impl Client {
    pub unsafe fn new(raw: &'static mut [u8]) -> Self {
        let regs = &mut *(raw.as_mut_ptr() as *mut ArgRegs);
        Self { raw, regs }
    }

    #[inline]
    pub unsafe fn load_args(&mut self) {
        todo!()
    }

    #[inline]
    pub unsafe fn ecall(&self) {
        asm!("ecall")
    }

    #[inline]
    pub fn save_output(&mut self) {
        todo!()
    }

    #[inline]
    pub fn resume(&self) {
        todo!()
    }
}
