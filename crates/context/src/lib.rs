#![no_std]
use core::arch::asm;

use riscv::register::{satp, scause, sepc, sie, sip, sscratch, stval, stvec};
use sbi::TrapRegs;

pub fn satp_from_bits(bits: usize) -> satp::Satp {
    let current = satp::read();
    satp::write(bits);
    let res = satp::read();
    satp::write(current.bits());
    res
}

#[inline]
pub fn sstatus_read_bit() -> usize {
    let sstatus;
    unsafe {
        asm!(
            "csrr {}, sstatus",
            out(reg) sstatus,
        );
    }
    sstatus
}

pub struct HartContext {
    pub tregs: TrapRegs,
    pub sregs: SupervisorRegs,
    pub pc: usize,
}

impl HartContext {
    pub fn save(&mut self, regs: &TrapRegs) {
        self.tregs = regs.clone();
        self.sregs = SupervisorRegs::dump();
    }

    /// The function will may change all smode registers
    pub unsafe fn restore(&self) -> TrapRegs {
        unsafe { self.sregs.write() };
        self.tregs.clone()
    }
}

// #[derive(Clone)]
// pub struct GeneralRegs {
//     pub zero: usize,
//     pub ra: usize,
//     pub sp: usize,
//     pub gp: usize,
//     pub tp: usize,
//     pub t0: usize,
//     pub t1: usize,
//     pub t2: usize,
//     pub s0: usize,
//     pub s1: usize,
//     pub a0: usize,
//     pub a1: usize,
//     pub a2: usize,
//     pub a3: usize,
//     pub a4: usize,
//     pub a5: usize,
//     pub a6: usize,
//     pub a7: usize,
//     pub s2: usize,
//     pub s3: usize,
//     pub s4: usize,
//     pub s5: usize,
//     pub s6: usize,
//     pub s7: usize,
//     pub s8: usize,
//     pub s9: usize,
//     pub s10: usize,
//     pub s11: usize,
//     pub t3: usize,
//     pub t4: usize,
//     pub t5: usize,
//     pub t6: usize,
// }

// impl GeneralRegs {
//     pub fn empty() -> Self {
//         Self {
//             zero: 0,
//             ra: 0,
//             sp: 0,
//             gp: 0,
//             tp: 0,
//             t0: 0,
//             t1: 0,
//             t2: 0,
//             s0: 0,
//             s1: 0,
//             a0: 0,
//             a1: 0,
//             a2: 0,
//             a3: 0,
//             a4: 0,
//             a5: 0,
//             a6: 0,
//             a7: 0,
//             s2: 0,
//             s3: 0,
//             s4: 0,
//             s5: 0,
//             s6: 0,
//             s7: 0,
//             s8: 0,
//             s9: 0,
//             s10: 0,
//             s11: 0,
//             t3: 0,
//             t4: 0,
//             t5: 0,
//             t6: 0,
//         }
//     }

//     pub fn from_trap_regs(regs: &TrapRegs) -> Self {
//         Self {
//             zero: regs.zero,
//             ra: regs.ra,
//             sp: regs.sp,
//             gp: regs.gp,
//             tp: regs.tp,
//             t0: regs.t0,
//             t1: regs.t1,
//             t2: regs.t2,
//             s0: regs.s0,
//             s1: regs.s1,
//             a0: regs.a0,
//             a1: regs.a1,
//             a2: regs.a2,
//             a3: regs.a3,
//             a4: regs.a4,
//             a5: regs.a5,
//             a6: regs.a6,
//             a7: regs.a7,
//             s2: regs.s2,
//             s3: regs.s3,
//             s4: regs.s4,
//             s5: regs.s5,
//             s6: regs.s6,
//             s7: regs.s7,
//             s8: regs.s8,
//             s9: regs.s9,
//             s10: regs.s10,
//             s11: regs.s11,
//             t3: regs.t3,
//             t4: regs.t4,
//             t5: regs.t5,
//             t6: regs.t6,
//         }
//     }
// }

pub struct SupervisorRegs {
    pub stvec: usize,
    // pub satp: usize,
    // pub satp: satp::Satp,
    pub satp: usize,
    pub sstatus: usize,
    pub sscratch: usize,
    pub sip: usize,
    pub sie: usize,
    // pub scounteren: usize,
    pub sepc: usize,
    pub scaues: usize,
    pub stval: usize,
    // pub senvcfg: usize,
}

impl SupervisorRegs {
    pub fn dump() -> Self {
        Self {
            stvec: stvec::read().bits(),
            satp: satp::read().bits(),
            sstatus: sstatus_read_bit(),
            sscratch: sscratch::read(),
            sip: sip::read().bits(),
            sie: sie::read().bits(),
            sepc: sepc::read(),
            scaues: scause::read().bits(),
            stval: stval::read(),
        }
    }

    pub unsafe fn write(&self) {
        unsafe {
            self.write_stvec();
            self.write_satp();
            self.write_sstatus();
            self.write_sscratch();
            self.write_sie();
            self.write_sip();
            self.write_sepc();
            self.write_scaues();
            self.write_stval();
        }
    }

    #[inline]
    pub fn write_sepc(&self) {
        sepc::write(self.sepc);
    }

    #[inline]
    pub unsafe fn write_scaues(&self) {
        unsafe { scause::write(self.scaues) };
    }

    #[inline]
    pub unsafe fn write_stval(&self) {
        unsafe { stval::write(self.stval) };
    }

    #[inline]
    pub fn write_satp(&self) {
        satp::write(self.satp);
    }

    #[inline]
    pub fn write_sscratch(&self) {
        sscratch::write(self.sscratch);
    }

    #[inline]
    pub unsafe fn write_stvec(&self) {
        unsafe {
            asm!(
                "csrw stvec, {}",
                in(reg) self.stvec,
            )
        }
    }

    #[inline]
    pub unsafe fn write_sstatus(&self) {
        unsafe {
            asm!(
                "csrw sstatus, {}",
                in(reg) self.sstatus,
            )
        }
    }

    #[inline]
    pub unsafe fn write_sie(&self) {
        unsafe {
            asm!(
                "csrw sie, {}",
                in(reg) self.sie,
            )
        }
    }

    #[inline]
    pub unsafe fn write_sip(&self) {
        unsafe {
            asm!(
                "csrw sip, {}",
                in(reg) self.sip,
            )
        }
    }
}

// fn read_scout

// pub struct Context {
//     pub gregs: GeneralRegs,
//     /// when the enclave is running, the sregs should be replace with host OS's supervisor registers.
//     /// Espeacially, `sscratch` register.
//     pub sregs: SupervisorRegs,
// }

// impl Context {
//     pub fn empty() -> Self {
//         Self {
//             gregs: GeneralRegs::empty(),
//             sregs: SupervisorRegs::empty(),
//         }
//     }
// }
