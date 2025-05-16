use riscv::register::{mcause, mtval, scause, sepc, stval, stvec};

pub const MSTATUS_SIE: usize = 0x00000002;
pub const MSTATUS_MIE: usize = 0x00000008;
pub const MSTATUS_SPIE: usize = 0x1 << 5;
pub const MSTATUS_MPIE: usize = 0x80;
pub const MSTATUS_SPP: usize = 0x1 << 8;
pub const MSTATUS_MPP_SHIFT: usize = 11;
pub const MSTATUS_MPP: usize = 0b11 << MSTATUS_MPP_SHIFT;

#[repr(C)]
#[derive(Clone)]
pub struct TrapRegs {
    pub zero: usize,
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
    pub mepc: usize,
    pub mstatus: usize,
    pub mstatush: usize,
}

impl TrapRegs {
    #[inline(always)]
    pub fn get_reg(&self, id: usize) -> usize {
        unsafe { *(self as *const TrapRegs as *const usize).add(id) }
    }

    #[inline(always)]
    pub unsafe fn enable_interrupt(&mut self) -> &mut Self {
        self.mstatus |= MSTATUS_MPIE;
        self.mstatus &= !MSTATUS_MIE;
        self
    }

    #[inline(always)]
    pub unsafe fn switch_next_mode(&mut self, next_mode: usize) -> &mut Self {
        self.mstatus &= !MSTATUS_SPIE;
        if self.mstatus & MSTATUS_SIE == MSTATUS_SIE {
            self.mstatus |= MSTATUS_SPIE;
        }
        // clean SIE
        self.mstatus &= !MSTATUS_SIE;
        self.mstatus &= !MSTATUS_MPP;
        self.mstatus |= next_mode;
        self
    }

    #[inline(always)]
    pub unsafe fn set_mepc(&mut self, mepc: usize) -> &mut Self {
        self.mepc = mepc;
        self
    }

    #[inline(always)]
    pub unsafe fn fix_mepc(&mut self, offset: usize) -> &mut Self {
        self.mepc += offset;
        self
    }

    pub unsafe fn redirect_to_smode(&mut self) -> &mut Self {
        self.mepc = stvec::read().bits();

        scause::write(mcause::read().bits());
        stval::write(mtval::read());
        sepc::write(self.mepc);

        self.enable_interrupt().switch_next_mode(MSTATUS_SPP)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct TrapRegsSMode {
    pub zero: usize,
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
    pub sepc: usize,
    pub sstatus: usize,
}

impl TrapRegsSMode {
    pub fn from_trapreg(trap_regs: TrapRegs) -> Self {
        TrapRegsSMode {
            zero: trap_regs.zero,
            ra: trap_regs.ra,
            sp: trap_regs.sp,
            gp: trap_regs.gp,
            tp: trap_regs.tp,
            t0: trap_regs.t0,
            t1: trap_regs.t1,
            t2: trap_regs.t2,
            s0: trap_regs.s0,
            s1: trap_regs.s1,
            a0: trap_regs.a0,
            a1: trap_regs.a1,
            a2: trap_regs.a2,
            a3: trap_regs.a3,
            a4: trap_regs.a4,
            a5: trap_regs.a5,
            a6: trap_regs.a6,
            a7: trap_regs.a7,
            s2: trap_regs.s2,
            s3: trap_regs.s3,
            s4: trap_regs.s4,
            s5: trap_regs.s5,
            s6: trap_regs.s6,
            s7: trap_regs.s7,
            s8: trap_regs.s8,
            s9: trap_regs.s9,
            s10: trap_regs.s10,
            s11: trap_regs.s11,
            t3: trap_regs.t3,
            t4: trap_regs.t4,
            t5: trap_regs.t5,
            t6: trap_regs.t6,
            sepc: 0,
            sstatus: 0,
        }
    }
}
