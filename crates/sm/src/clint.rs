use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicPtr, Ordering},
};

use riscv::register::mhartid;

#[repr(transparent)]
pub struct MTIME(UnsafeCell<u64>);

#[repr(transparent)]
pub struct MTIMECMP(UnsafeCell<u64>);

#[repr(transparent)]
pub struct MSIP(UnsafeCell<u32>);

#[repr(transparent)]
pub struct SETSSIP(UnsafeCell<u32>);

#[repr(transparent)]
pub struct MTIMER([MTIMECMP; 4095]);

#[repr(transparent)]
pub struct MSWI([MSIP; 4095]);

#[repr(transparent)]
pub struct SSWI([SETSSIP; 4095]);

pub struct ClintClient {
    clint: AtomicPtr<Clint>,
    hart_num: usize,
}

impl ClintClient {
    pub fn new(hart_num: usize) -> Self {
        Self {
            clint: AtomicPtr::new(core::ptr::null_mut()),
            hart_num,
        }
    }

    pub fn init(&mut self, base: *const u8) {
        self.clint = AtomicPtr::new(base as _);
    }

    pub fn send_ipi_other_harts(&self) {
        let clint = unsafe { &*self.clint.load(Ordering::Relaxed) };
        for i in 0..self.hart_num {
            if i == mhartid::read() {
                continue;
            }
            clint.set_msip(i);
        }
    }

    pub fn reset_msip(&self) {
        unsafe { &*self.clint.load(Ordering::Relaxed) }.clear_msip(mhartid::read());
    }
}

#[repr(C)]
pub struct Clint {
    mswi: MSWI,
    reserve: u32,
    mtimer: MTIMER,
    mtime: MTIME,
}

impl Clint {
    pub fn clear_msip(&self, hartid: usize) {
        unsafe { self.mswi.0[hartid].0.get().write_volatile(0) }
    }

    pub fn set_msip(&self, hartid: usize) {
        unsafe { self.mswi.0[hartid].0.get().write_volatile(1) }
    }
}
