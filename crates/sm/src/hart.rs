use core::cell::UnsafeCell;

use enclave::{EncListNode, EnclaveId, EnclaveIdx, LinuxUserEnclave};
use riscv::register::{
    mcause::Exception, mepc, mhartid, mstatus, satp, scause, sepc, sscratch, sstatus, stvec,
};
use sbi::{TrapRegs, TrapRegsSMode};
use spin::Mutex;
use vm::consts::PAGE_SIZE;

use crate::{Error, consts::MAX_HART_NUM, hart, pmp::reset_pmp_registers};

const DEFAULT_HART: UnsafeHartState = UnsafeHartState::new();
static ALL_HARTS: [UnsafeHartState; MAX_HART_NUM] = [DEFAULT_HART; MAX_HART_NUM];

const EMPTY_OP: Mutex<HartStateOps> = Mutex::new(HartStateOps::empty());
static OPS: [Mutex<HartStateOps>; MAX_HART_NUM] = [EMPTY_OP; MAX_HART_NUM];

pub const MSTATUS_SIE: usize = 0x00000002;
pub const MSTATUS_MIE: usize = 0x00000008;
pub const MSTATUS_SPIE: usize = 0x1 << 5;
pub const MSTATUS_MPIE: usize = 0x80;
pub const MSTATUS_SPP: usize = 0x1 << 8;
pub const MSTATUS_MPP_SHIFT: usize = 11;
pub const MSTATUS_MPP: usize = 0x2 << MSTATUS_MPP_SHIFT;

/// Cross hart access is not allowed
pub fn current() -> &'static mut HartState {
    ALL_HARTS[mhartid::read()].as_mut()
}

pub fn send_ops(id: usize, ops: HartStateOps) {
    *OPS[id].lock() = ops;
}

pub fn recv_op(id: usize) -> HartStateOps {
    OPS[id].lock().clone()
}

pub fn take_op() -> HartStateOps {
    let op = &mut *OPS[mhartid::read()].lock();
    core::mem::replace(op, HartStateOps::empty())
}

pub struct UnsafeHartState(UnsafeCell<HartState>);

unsafe impl Sync for UnsafeHartState {}
unsafe impl Send for UnsafeHartState {}

impl UnsafeHartState {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(HartState::new()))
    }

    pub fn as_mut(&self) -> &mut HartState {
        unsafe { &mut *self.0.get() }
    }
}

#[derive(Clone)]
pub struct HartState {
    pub enclave: EnclaveIdx,
}

impl Default for HartState {
    fn default() -> Self {
        Self {
            enclave: EnclaveIdx::HOST,
        }
    }
}

impl HartState {
    pub const fn new() -> Self {
        Self {
            enclave: EnclaveIdx::HOST,
        }
    }

    // pub fn get_enc_ptr(&self) -> Option<&mut EncListNode> {
    //     if self.enclave == EnclaveIdx::HOST {
    //         None
    //     } else {
    //         Some(EncListNode::from_idx(self.enclave))
    //     }
    // }

    pub fn set_enc(&mut self, idx: impl Into<EnclaveIdx>) {
        let idx = idx.into();
        self.enclave = idx;
    }

    pub fn exit_enclave(&mut self) {
        self.switch_context();
        self.enclave = EnclaveIdx::HOST;
    }

    pub fn get_idx(&self) -> EnclaveIdx {
        self.enclave
    }

    // pub fn get_eid(&self) -> EnclaveId {
    //     match self.enclave {
    //         EnclaveIdx::HOST => EnclaveId::HOST,
    //         _ => self.get_enc_ptr().unwrap().get_id(),
    //     }
    // }

    pub unsafe fn enter_enclave(
        &mut self,
        idx: EnclaveIdx,
        next_addr: usize,
        arg0: usize,
        arg1: usize,
        next_sp: usize,
        next_satp: satp::Satp,
    ) -> Result<!, Error> {
        use riscv::register::satp;

        satp::write(next_satp.bits());
        self.enclave = idx;

        self.switch_context();
        unsafe { self.call(mstatus::MPP::Supervisor, next_addr, arg0, arg1, next_sp) }
    }

    // pub unsafe fn enter_lde(&mut self, enc: &mut LinuxDriverEnclave, tregs: &mut TrapRegs) {
    //     self.enclave = enc.idx();
    //     self.switch_context();
    //     // unsafe { self.call(mstatus::MPP::Supervisor, enc.enclave_ctx.sregs.satp, ) }
    // }

    pub fn clean_pmp(&mut self) {
        reset_pmp_registers();
    }

    fn switch_context(&mut self) {
        self.clean_pmp();
    }

    pub unsafe fn call(
        &mut self,
        next_mode: mstatus::MPP,
        next_addr: usize,
        arg0: usize,
        arg1: usize,
        next_sp: usize,
    ) -> ! {
        use core::arch::asm;
        use riscv::asm::sfence_vma_all;
        use riscv::register::*;

        unsafe {
            mstatus::set_mpp(next_mode.into());
            sstatus::clear_sie(); // disable SIE
            mepc::write(next_addr);
            sfence_vma_all();

            asm!(
                "mv     sp, {}",
                "mret",
                in(reg) next_sp,
                in("a0") arg0,
                in("a1") arg1,
                options(noreturn),
            );
        }
    }

    pub unsafe fn redirect_ecall(&mut self, target: EnclaveIdx, tregs: &mut TrapRegs, sepc: usize) {
        // use enclave::EnclaveRef;

        // let stvec = target.enclave_ctx.sregs.stvec;
        // let stvec = match EnclaveRef::from_idx(target) {
        //     EnclaveRef::User(enclave) => enclave.enclave_ctx.sregs.stvec,
        //     EnclaveRef::Driver(enclave) => enclave.enclave_ctx.sregs.stvec,
        // };
        // let satp = match EnclaveRef::from_idx(target) {
        //     EnclaveRef::User(enclave) => enclave.enclave_ctx.sregs.satp,
        //     EnclaveRef::Driver(enclave) => enclave.enclave_ctx.sregs.satp,
        // };
        // let sscratch = match EnclaveRef::from_idx(target) {
        //     EnclaveRef::User(enclave) => enclave.enclave_ctx.sregs.sscratch,
        //     EnclaveRef::Driver(enclave) => enclave.enclave_ctx.sregs.sscratch,
        // };
        // let sp = match EnclaveRef::from_idx(target) {
        //     EnclaveRef::User(enclave) => enclave.enclave_ctx.tregs.sp,
        //     EnclaveRef::Driver(enclave) => enclave.enclave_ctx.tregs.sp,
        // };

        // let ext_satp = satp::read();

        unsafe {
            mstatus::set_mpp(mstatus::MPP::Supervisor);

            // enable interrupts
            // set MPIE and clear MIE
            tregs.mstatus |= MSTATUS_MPIE;
            tregs.mstatus &= !MSTATUS_MIE;

            // set SPIE
            tregs.mstatus &= !MSTATUS_SPIE;
            if tregs.mstatus & MSTATUS_SIE == MSTATUS_SIE {
                tregs.mstatus |= MSTATUS_SPIE;
            }

            // clean SIE
            tregs.mstatus &= !MSTATUS_SIE;

            // let smode know the epc
            sepc::write(sepc);
            // satp::write(satp.bits());
            // sscratch::write(sscratch);
            // stvec::write(stvec, stvec::TrapMode::Direct);
            // scause::write(Exception::UserEnvCall as usize);

            // go to the lde traphandler
            tregs.mepc = stvec::read().bits();
        }
    }

    pub unsafe fn return_to_smode(&mut self, tregs: &mut TrapRegs, is_ecall: bool) {
        use riscv::register::*;

        tregs.mepc += if is_ecall { 0x4 } else { 0x2 };

        tregs.mstatus &= !MSTATUS_MPP;
        tregs.mstatus |= (mstatus::MPP::Supervisor as usize) << MSTATUS_MPP_SHIFT;
    }

    pub unsafe fn switch_mode(&mut self, mepc: usize, next_mode: mstatus::MPP) {
        unsafe {
            mstatus::set_mpp(next_mode.into());

            // enable interrupts
            mstatus::set_mpie();
            mstatus::clear_mie();

            mepc::write(mepc);
        }
    }

    // pub unsafe fn return_to_smode(&mut self, )

    // unsafe fn jump(&mut self, next_mode: mstatus::MPP, next_addr: usize, next_satp: usize) {
    //     use core::arch::asm;
    //     use riscv::asm::sfence_vma_all;
    //     use riscv::register::*;

    //     unsafe {
    //         mstatus::set_mpp(next_mode.into());
    //         sstatus::clear_sie(); // disable SIE
    //         mepc::write(next_addr);
    //         satp::write(next_satp);
    //         sfence_vma_all();

    //         asm!("mret", options(noreturn),);
    //     }
    // }
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
