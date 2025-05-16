#[macro_export]
macro_rules! trap_regs_offset {
    ($reg: ident) => {
        core::mem::offset_of!($crate::TrapRegs, $reg) as isize
    };
}

#[macro_export]
macro_rules! trap_regs_offset_smode {
    ($reg: ident) => {
        core::mem::offset_of!($crate::TrapRegsSMode, $reg) as isize
    };
}

#[macro_export]
macro_rules! assign_same_fields {
    ($src:expr, $dest:expr, [ $( $field:ident ),* $(,)* ]) => {
        $(
            $dest.$field = $src.$field;
        )*
    }
}


#[macro_export]
macro_rules! trap_save_and_setup_sp_t0 {
    () => {
        core::arch::asm!(
            "csrrw  tp, mscratch, tp",
            "sd     t0, {0}(tp)",
            "csrr   t0, mstatus",
            "srl    t0, t0, 11",
            "and	t0, t0, 3",
            "slti	t0, t0, 3",
            "add	t0, t0, -1",
            "xor	sp, sp, tp",
            "and    t0, t0, sp",
            "xor    sp, sp, tp",
            "xor    t0, tp, t0",
            "sd     sp, {1}(t0)",
            "add    sp, t0, -({2})",
            "ld     t0, {0}(tp)",
            "sd     t0, {3}(sp)",
            "csrrw  tp, mscratch, tp",
            const core::mem::offset_of!($crate::Scratch, tmp0),
            const ($crate::trap_regs_offset!(sp) - core::mem::size_of::<$crate::TrapRegs>() as isize),
            const core::mem::size_of::<$crate::TrapRegs>(),
            const $crate::trap_regs_offset!(t0),
        )
    };
}

#[macro_export]
macro_rules! trap_save_mepc_mstatus {
    () => {
        core::arch::asm!(
            "csrr   t0, mepc",
            "sd     t0, {}(sp)",
            "csrr   t0, mstatus",
            "sd     t0, {}(sp)",
            "sd     zero, {}(sp)",
            const $crate::trap_regs_offset!(mepc),
            const $crate::trap_regs_offset!(mstatus),
            const $crate::trap_regs_offset!(mstatush),
        )
    };
}

#[macro_export]
macro_rules! trap_save_general_regs_except_sp_t0 {
    () => {
        core::arch::asm!(
            "sd     zero, {}(sp)",
            "sd     ra, {}(sp)",
            "sd     gp, {}(sp)",
            "sd     tp, {}(sp)",
            "sd     t1, {}(sp)",
            "sd     t2, {}(sp)",
            "sd     s0, {}(sp)",
            "sd     s1, {}(sp)",
            "sd     a0, {}(sp)",
            "sd     a1, {}(sp)",
            "sd     a2, {}(sp)",
            "sd     a3, {}(sp)",
            "sd     a4, {}(sp)",
            "sd     a5, {}(sp)",
            "sd     a6, {}(sp)",
            "sd     a7, {}(sp)",
            "sd     s2, {}(sp)",
            "sd     s3, {}(sp)",
            "sd     s4, {}(sp)",
            "sd     s5, {}(sp)",
            "sd     s6, {}(sp)",
            "sd     s7, {}(sp)",
            "sd     s8, {}(sp)",
            "sd     s9, {}(sp)",
            "sd     s10, {}(sp)",
            "sd     s11, {}(sp)",
            "sd     t3, {}(sp)",
            "sd     t4, {}(sp)",
            "sd     t5, {}(sp)",
            "sd     t6, {}(sp)",
            const $crate::trap_regs_offset!(zero),
            const $crate::trap_regs_offset!(ra),
            const $crate::trap_regs_offset!(gp),
            const $crate::trap_regs_offset!(tp),
            const $crate::trap_regs_offset!(t1),
            const $crate::trap_regs_offset!(t2),
            const $crate::trap_regs_offset!(s0),
            const $crate::trap_regs_offset!(s1),
            const $crate::trap_regs_offset!(a0),
            const $crate::trap_regs_offset!(a1),
            const $crate::trap_regs_offset!(a2),
            const $crate::trap_regs_offset!(a3),
            const $crate::trap_regs_offset!(a4),
            const $crate::trap_regs_offset!(a5),
            const $crate::trap_regs_offset!(a6),
            const $crate::trap_regs_offset!(a7),
            const $crate::trap_regs_offset!(s2),
            const $crate::trap_regs_offset!(s3),
            const $crate::trap_regs_offset!(s4),
            const $crate::trap_regs_offset!(s5),
            const $crate::trap_regs_offset!(s6),
            const $crate::trap_regs_offset!(s7),
            const $crate::trap_regs_offset!(s8),
            const $crate::trap_regs_offset!(s9),
            const $crate::trap_regs_offset!(s10),
            const $crate::trap_regs_offset!(s11),
            const $crate::trap_regs_offset!(t3),
            const $crate::trap_regs_offset!(t4),
            const $crate::trap_regs_offset!(t5),
            const $crate::trap_regs_offset!(t6),
        )
    };
}

#[macro_export]
macro_rules! clear_genera_regs {
    () => {
        core::arch::asm!(
            "mv     t1, zero",
            "mv     t2, zero",
            "mv     s0, zero",
            "mv     s1, zero",
            "mv     a0, zero",
            "mv     a1, zero",
            "mv     a2, zero",
            "mv     a3, zero",
            "mv     a4, zero",
            "mv     a5, zero",
            "mv     a6, zero",
            "mv     a7, zero",
            "mv     s2, zero",
            "mv     s3, zero",
            "mv     s4, zero",
            "mv     s5, zero",
            "mv     s6, zero",
            "mv     s7, zero",
            "mv     s8, zero",
            "mv     s9, zero",
            "mv     s10, zero",
            "mv     s11, zero",
            "mv     t3, zero",
            "mv     t4, zero",
            "mv     t5, zero",
            "mv     t6, zero",
        )
    };
}

#[macro_export]
macro_rules! call_trap_handler {
    ($handle:path) => {
        core::arch::asm!(
            "add    a0,sp,zero",
            "call   {}",
            sym $handle
        )
    };
    () => {
        core::arch::asm!(
            "add    a0,sp,zero"
        )
    }
}

#[macro_export]
macro_rules! trap_restore_general_regs_except_a0_t0 {
    () => {
        core::arch::asm!(
            "ld     ra, {}(a0)",
            "ld     sp, {}(a0)",
            "ld     gp, {}(a0)",
            "ld     tp, {}(a0)",
            "ld     t1, {}(a0)",
            "ld     t2, {}(a0)",
            "ld     s0, {}(a0)",
            "ld     s1, {}(a0)",
            "ld     a1, {}(a0)",
            "ld     a2, {}(a0)",
            "ld     a3, {}(a0)",
            "ld     a4, {}(a0)",
            "ld     a5, {}(a0)",
            "ld     a6, {}(a0)",
            "ld     a7, {}(a0)",
            "ld     s2, {}(a0)",
            "ld     s3, {}(a0)",
            "ld     s4, {}(a0)",
            "ld     s5, {}(a0)",
            "ld     s6, {}(a0)",
            "ld     s7, {}(a0)",
            "ld     s8, {}(a0)",
            "ld     s9, {}(a0)",
            "ld     s10, {}(a0)",
            "ld     s11, {}(a0)",
            "ld     t3, {}(a0)",
            "ld     t4, {}(a0)",
            "ld     t5, {}(a0)",
            "ld     t6, {}(a0)",
            const $crate::trap_regs_offset!(ra),
            const $crate::trap_regs_offset!(sp),
            const $crate::trap_regs_offset!(gp),
            const $crate::trap_regs_offset!(tp),
            const $crate::trap_regs_offset!(t1),
            const $crate::trap_regs_offset!(t2),
            const $crate::trap_regs_offset!(s0),
            const $crate::trap_regs_offset!(s1),
            const $crate::trap_regs_offset!(a1),
            const $crate::trap_regs_offset!(a2),
            const $crate::trap_regs_offset!(a3),
            const $crate::trap_regs_offset!(a4),
            const $crate::trap_regs_offset!(a5),
            const $crate::trap_regs_offset!(a6),
            const $crate::trap_regs_offset!(a7),
            const $crate::trap_regs_offset!(s2),
            const $crate::trap_regs_offset!(s3),
            const $crate::trap_regs_offset!(s4),
            const $crate::trap_regs_offset!(s5),
            const $crate::trap_regs_offset!(s6),
            const $crate::trap_regs_offset!(s7),
            const $crate::trap_regs_offset!(s8),
            const $crate::trap_regs_offset!(s9),
            const $crate::trap_regs_offset!(s10),
            const $crate::trap_regs_offset!(s11),
            const $crate::trap_regs_offset!(t3),
            const $crate::trap_regs_offset!(t4),
            const $crate::trap_regs_offset!(t5),
            const $crate::trap_regs_offset!(t6),
        )
    };
}

#[macro_export]
macro_rules! trap_restore_mepc_mstatus {
    () => {
        core::arch::asm!(
            "ld     t0, {}(a0)",
            "csrw   mepc, t0",
            "ld     t0, {}(a0)",
            "csrw   mstatus, t0",
            const $crate::trap_regs_offset!(mepc),
            const $crate::trap_regs_offset!(mstatus),
        )
    };
}

#[macro_export]
macro_rules! trap_restore_a0_t0 {
    () => {
        core::arch::asm!(
            "ld     t0, {}(a0)",
            "ld     a0, {}(a0)",
            const $crate::trap_regs_offset!(t0),
            const $crate::trap_regs_offset!(a0),
        )
    };
}

macro_rules! for_each_pmpcfg {
    () => {
        // riscv::registers::pmp
    };
}

#[macro_export]
macro_rules! trap_save_and_setup_sp_t0_smode {
    () => {
        core::arch::asm!(
            "csrrw  tp, sscratch, tp",
            "sd     t0, {0}(tp)",
            "csrr   t0, sstatus",
            "srl    t0, t0, 8",
            "and	t0, t0, 1",
            "xor    t0, t0, -1"
            "addi	t0, t0, 1",
            "xor	sp, sp, tp",
            "and    t0, t0, sp",
            "xor    sp, sp, tp",
            "xor    t0, tp, t0",
            "sd     sp, {1}(t0)",
            "add    sp, t0, -({2})",
            "ld     t0, {0}(tp)",
            "sd     t0, {3}(sp)",
            "csrrw  tp, sscratch, tp",
            const core::mem::offset_of!($crate::Scratch, tmp0),
            const ($crate::trap_regs_offset_Smode!(sp) - core::mem::size_of::<$crate::TrapRegsSMode>() as isize),
            const core::mem::size_of::<$crate::TrapRegsSMode>(),
            const $crate::trap_regs_offset_Smode!(t0),
        )
    };
}

#[macro_export]
macro_rules! trap_save_sepc_sstatus {
    () => {
        core::arch::asm!(
            "csrr   t0, sepc",
            "sd     t0, {}(sp)",
            "csrr   t0, sstatus",
            "sd     t0, {}(sp)",
            "sd     zero, {}(sp)",
            const $crate::trap_regs_offset_Smode!(sepc),
            const $crate::trap_regs_offset_Smode!(sstatus),
            const $crate::trap_regs_offset_Smode!(sstatush),
        )
    };
}

#[macro_export]
macro_rules! trap_save_general_regs_except_sp_t0_smode {
    () => {
        core::arch::asm!(
            "sd     zero, {}(sp)",
            "sd     ra, {}(sp)",
            "sd     gp, {}(sp)",
            "sd     tp, {}(sp)",
            "sd     t1, {}(sp)",
            "sd     t2, {}(sp)",
            "sd     s0, {}(sp)",
            "sd     s1, {}(sp)",
            "sd     a0, {}(sp)",
            "sd     a1, {}(sp)",
            "sd     a2, {}(sp)",
            "sd     a3, {}(sp)",
            "sd     a4, {}(sp)",
            "sd     a5, {}(sp)",
            "sd     a6, {}(sp)",
            "sd     a7, {}(sp)",
            "sd     s2, {}(sp)",
            "sd     s3, {}(sp)",
            "sd     s4, {}(sp)",
            "sd     s5, {}(sp)",
            "sd     s6, {}(sp)",
            "sd     s7, {}(sp)",
            "sd     s8, {}(sp)",
            "sd     s9, {}(sp)",
            "sd     s10, {}(sp)",
            "sd     s11, {}(sp)",
            "sd     t3, {}(sp)",
            "sd     t4, {}(sp)",
            "sd     t5, {}(sp)",
            "sd     t6, {}(sp)",
            const $crate::trap_regs_offset_Smode!(zero),
            const $crate::trap_regs_offset_Smode!(ra),
            const $crate::trap_regs_offset_Smode!(gp),
            const $crate::trap_regs_offset_Smode!(tp),
            const $crate::trap_regs_offset_Smode!(t1),
            const $crate::trap_regs_offset_Smode!(t2),
            const $crate::trap_regs_offset_Smode!(s0),
            const $crate::trap_regs_offset_Smode!(s1),
            const $crate::trap_regs_offset_Smode!(a0),
            const $crate::trap_regs_offset_Smode!(a1),
            const $crate::trap_regs_offset_Smode!(a2),
            const $crate::trap_regs_offset_Smode!(a3),
            const $crate::trap_regs_offset_Smode!(a4),
            const $crate::trap_regs_offset_Smode!(a5),
            const $crate::trap_regs_offset_Smode!(a6),
            const $crate::trap_regs_offset_Smode!(a7),
            const $crate::trap_regs_offset_Smode!(s2),
            const $crate::trap_regs_offset_Smode!(s3),
            const $crate::trap_regs_offset_Smode!(s4),
            const $crate::trap_regs_offset_Smode!(s5),
            const $crate::trap_regs_offset_Smode!(s6),
            const $crate::trap_regs_offset_Smode!(s7),
            const $crate::trap_regs_offset_Smode!(s8),
            const $crate::trap_regs_offset_Smode!(s9),
            const $crate::trap_regs_offset_Smode!(s10),
            const $crate::trap_regs_offset_Smode!(s11),
            const $crate::trap_regs_offset_Smode!(t3),
            const $crate::trap_regs_offset_Smode!(t4),
            const $crate::trap_regs_offset_Smode!(t5),
            const $crate::trap_regs_offset_Smode!(t6),
        )
    };
}

#[macro_export]
macro_rules! trap_restore_general_regs_except_a0_t0_smode {
    () => {
        core::arch::asm!(
            "ld     ra, {}(a0)",
            "ld     sp, {}(a0)",
            "ld     gp, {}(a0)",
            "ld     tp, {}(a0)",
            "ld     t1, {}(a0)",
            "ld     t2, {}(a0)",
            "ld     s0, {}(a0)",
            "ld     s1, {}(a0)",
            "ld     a1, {}(a0)",
            "ld     a2, {}(a0)",
            "ld     a3, {}(a0)",
            "ld     a4, {}(a0)",
            "ld     a5, {}(a0)",
            "ld     a6, {}(a0)",
            "ld     a7, {}(a0)",
            "ld     s2, {}(a0)",
            "ld     s3, {}(a0)",
            "ld     s4, {}(a0)",
            "ld     s5, {}(a0)",
            "ld     s6, {}(a0)",
            "ld     s7, {}(a0)",
            "ld     s8, {}(a0)",
            "ld     s9, {}(a0)",
            "ld     s10, {}(a0)",
            "ld     s11, {}(a0)",
            "ld     t3, {}(a0)",
            "ld     t4, {}(a0)",
            "ld     t5, {}(a0)",
            "ld     t6, {}(a0)",
            const $crate::trap_regs_offset_Smode!(ra),
            const $crate::trap_regs_offset_Smode!(sp),
            const $crate::trap_regs_offset_Smode!(gp),
            const $crate::trap_regs_offset_Smode!(tp),
            const $crate::trap_regs_offset_Smode!(t1),
            const $crate::trap_regs_offset_Smode!(t2),
            const $crate::trap_regs_offset_Smode!(s0),
            const $crate::trap_regs_offset_Smode!(s1),
            const $crate::trap_regs_offset_Smode!(a1),
            const $crate::trap_regs_offset_Smode!(a2),
            const $crate::trap_regs_offset_Smode!(a3),
            const $crate::trap_regs_offset_Smode!(a4),
            const $crate::trap_regs_offset_Smode!(a5),
            const $crate::trap_regs_offset_Smode!(a6),
            const $crate::trap_regs_offset_Smode!(a7),
            const $crate::trap_regs_offset_Smode!(s2),
            const $crate::trap_regs_offset_Smode!(s3),
            const $crate::trap_regs_offset_Smode!(s4),
            const $crate::trap_regs_offset_Smode!(s5),
            const $crate::trap_regs_offset_Smode!(s6),
            const $crate::trap_regs_offset_Smode!(s7),
            const $crate::trap_regs_offset_Smode!(s8),
            const $crate::trap_regs_offset_Smode!(s9),
            const $crate::trap_regs_offset_Smode!(s10),
            const $crate::trap_regs_offset_Smode!(s11),
            const $crate::trap_regs_offset_Smode!(t3),
            const $crate::trap_regs_offset_Smode!(t4),
            const $crate::trap_regs_offset_Smode!(t5),
            const $crate::trap_regs_offset_Smode!(t6),
        )
    };
}

#[macro_export]
macro_rules! trap_restore_sepc_sstatus {
    () => {
        core::arch::asm!(
            "ld     t0, {}(a0)",
            "csrw   sepc, t0",
            "ld     t0, {}(a0)",
            "csrw   sstatus, t0",
            const $crate::trap_regs_offset_Smode!(sepc),
            const $crate::trap_regs_offset_Smode!(sstatus),
        )
    };
}

#[macro_export]
macro_rules! trap_restore_a0_t0_smode {
    () => {
        core::arch::asm!(
            "ld     t0, {}(a0)",
            "ld     a0, {}(a0)",
            const $crate::trap_regs_offset_Smode!(t0),
            const $crate::trap_regs_offset_Smode!(a0),
        )
    };
}

#[macro_export]
macro_rules! tlb_flush {
    () => {
        core::arch::asm!(
            "fence.i",
            "sfence.vma",
        )
    };
}
