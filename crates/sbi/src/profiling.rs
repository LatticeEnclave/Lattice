use riscv::register::{cycle, instret, mcounteren};

/// set the mcounteren in M-mode to enable the access of S-mode
/// to cycle, time, instret
pub unsafe fn enable_s_mode_profile() {
    mcounteren::set_cy();
    mcounteren::set_tm();
    mcounteren::set_ir();
}

pub unsafe fn enable_cycle() {}

/// the three virtual registerscan be accessed after set mcounteren
/// and can be used in M mode with no extra modification
#[inline(always)]
pub fn get_time() -> usize {
    let val: usize;
    unsafe { core::arch::asm!("csrr a0, time", out("a0") val) }
    val
}

pub fn get_instret() -> usize {
    instret::read()
}

#[inline]
pub fn get_cycle() -> usize {
    cycle::read()
}
