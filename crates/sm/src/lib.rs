#![no_std]
#![feature(naked_functions)]
#![feature(never_type)]

use core::mem::MaybeUninit;

use htee_console::println;
use riscv::{asm::wfi, register::mscratch};

pub use init::init;
pub use platform::Platform;
pub use pma::{Owner, PhysMemArea, PhysMemAreaMgr, PmaProp};
pub use pmp::{PMP_COUNT, PmpStatus};
pub use sm::SecMonitor;
pub use trap_proxy::{TrapProxy, TrapRegs};

pub use device::DeviceInfo;
pub use error::Error;

pub mod consts;
mod device;
mod ecall;
mod error;
mod helper;
mod init;
mod sm;
mod trap;

mod enclave;

pub static mut SM: MaybeUninit<SecMonitor> = MaybeUninit::uninit();

#[inline(always)]
pub fn sm() -> &'static sm::SecMonitor {
    #[allow(static_mut_refs)]
    unsafe {
        SM.assume_init_ref()
    }
}

#[inline]
pub fn handle_panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    println!("{panic}");
    loop {
        wfi();
    }
}

#[inline]
pub fn check_stack_overflow() {
    #[inline(always)]
    fn get_sp() -> usize {
        let sp: usize;
        unsafe {
            core::arch::asm!("mv  {}, sp", out(reg) sp);
        }
        sp
    }

    debug_assert!(get_sp() > mscratch::read() - 0x1000);
}
