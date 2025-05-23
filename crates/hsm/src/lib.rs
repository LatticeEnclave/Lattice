#![no_std]

use core::{cell::UnsafeCell, ptr::NonNull};

use heapless::Vec;
use pmp::{MAX_PMP_COUNT, PmpBuf, PmpHelper};
use riscv::{
    asm::sfence_vma_all,
    register::{mhartid, mstatus},
};
// use sbi::TrapRegs;
use spin::Mutex;

pub const MAX_HART_NUM: usize = 16;

const DEFAULT_HART: UnsafeHartState = UnsafeHartState::new();
// static ALL_HARTS: [UnsafeHartState; MAX_HART_NUM] = [DEFAULT_HART; MAX_HART_NUM];

const EMPTY_OP: Mutex<HartStateOps> = Mutex::new(HartStateOps::empty());
// static OPS: [Mutex<HartStateOps>; MAX_HART_NUM] = [EMPTY_OP; MAX_HART_NUM];

// static HART_NUM: Once<usize> = Once::new();

pub const MSTATUS_SIE: usize = 0x00000002;
pub const MSTATUS_MIE: usize = 0x00000008;
pub const MSTATUS_SPIE: usize = 0x1 << 5;
pub const MSTATUS_MPIE: usize = 0x80;
pub const MSTATUS_SPP: usize = 0x1 << 8;
pub const MSTATUS_MPP_SHIFT: usize = 11;
pub const MSTATUS_MPP: usize = 0x2 << MSTATUS_MPP_SHIFT;

pub struct Hsm {
    hart_num: usize,
    all_harts: [UnsafeHartState; MAX_HART_NUM],
    ops: [Mutex<HartStateOps>; MAX_HART_NUM],
}

impl Hsm {
    pub fn new(hart_num: usize) -> Self {
        Self {
            hart_num,
            all_harts: [DEFAULT_HART; MAX_HART_NUM],
            ops: [EMPTY_OP; MAX_HART_NUM],
        }
    }

    pub fn current(&self) -> &mut HartState {
        self.all_harts[mhartid::read()].as_mut()
    }

    pub fn set_num(&mut self, num: usize) {
        self.hart_num = num;
    }

    pub fn send_ops(&self, id: usize, ops: HartStateOps) {
        *self.ops[id].lock() = ops;
    }

    pub fn recv_op(&self, id: usize) -> HartStateOps {
        self.ops[id].lock().clone()
    }

    #[inline]
    pub fn take_op(&self) -> HartStateOps {
        let op = &mut *self.ops[mhartid::read()].lock();
        core::mem::replace(op, HartStateOps::empty())
    }

    pub fn num(&self) -> usize {
        self.hart_num
    }

    #[inline]
    pub unsafe fn iter_hs_mut(&mut self) -> impl Iterator<Item = &mut HartState> {
        self.all_harts.iter_mut().map(|hs| hs.as_mut())
    }

    #[inline(always)]
    pub unsafe fn flush_tlb(&self) {
        // unsafe {
        //     // sfence_vma(satp::read().asid(), 0);
        // fence_i();
        sfence_vma_all();
        // };
    }

    pub unsafe fn mret(
        &self,
        addr: usize,
        mode: mstatus::MPP,
        arg0: usize,
        arg1: usize,
        sp: usize,
        satp: usize,
    ) -> ! {
        use core::arch::asm;
        // use riscv::asm::sfence_vma_all;
        use riscv::register::*;

        unsafe {
            mstatus::set_mpp(mode);
            mstatus::set_mpie();
            mstatus::clear_mie();
            mstatus::clear_sie(); // disable SIE
            mepc::write(addr);

            self.flush_tlb();

            asm!(
                "mv     sp, {}",
                "mret",
                in(reg) sp,
                in("a0") arg0,
                in("a1") arg1,
                in("a2") satp,
                in("a6") 0,
                in("a7") 0,
                options(noreturn),
            );
        }
    }
}

pub struct HartState {
    priv_data_ptr: Option<NonNull<u8>>,
    pub pmp_buf: NonNull<PmpBuf>,
}

impl Default for HartState {
    fn default() -> Self {
        Self::const_new()
    }
}

impl HartState {
    pub const fn const_new() -> Self {
        Self {
            priv_data_ptr: None,
            pmp_buf: NonNull::dangling(),
            // buf: Buffer::empty(),
        }
    }

    #[inline]
    pub fn get_priv<T: From<NonNull<u8>>>(&self) -> Option<T> {
        self.priv_data_ptr.map(T::from)
    }

    #[inline]
    pub fn set_priv<T: Into<NonNull<u8>>>(&mut self, ptr: T) {
        self.priv_data_ptr = Some(ptr.into());
    }

    #[inline]
    pub fn clear_priv(&mut self) {
        self.priv_data_ptr = None;
    }

    #[inline]
    pub fn clean_pmp(&self) {
        pmp::reset_pmp_registers();
    }

    #[inline]
    pub fn in_nw(&self) -> bool {
        self.priv_data_ptr == None
    }
}

#[derive(Clone)]
pub struct HartStateOps {
    pub clean_pmp: bool,
}

impl Default for HartStateOps {
    fn default() -> Self {
        Self::empty()
    }
}

impl HartStateOps {
    pub const fn empty() -> Self {
        Self { clean_pmp: false }
    }
}

pub struct UnsafeHartState(UnsafeCell<HartState>);

unsafe impl Sync for UnsafeHartState {}
unsafe impl Send for UnsafeHartState {}

impl UnsafeHartState {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(HartState::const_new()))
    }

    pub fn as_mut(&self) -> &mut HartState {
        unsafe { &mut *self.0.get() }
    }
}

pub struct Buffer {
    pub pmp: Vec<PmpHelper, MAX_PMP_COUNT>,
}

impl Buffer {
    pub fn get_pmp_mut(&mut self) -> &mut Vec<PmpHelper, MAX_PMP_COUNT> {
        &mut self.pmp
    }

    pub const fn empty() -> Self {
        Self { pmp: Vec::new() }
    }
}
