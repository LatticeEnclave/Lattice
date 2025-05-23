#![no_std]
#![no_main]

use core::arch::asm;
use sm::Platform;

pub struct General {}

impl Platform for General {}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start(next_addr: usize, arg1: usize) -> ! {
    unsafe {
        asm!(
            // use sbi stack
            "csrr sp, mscratch"
        );
        sm::init(&General {}, next_addr, arg1)
    }
}

#[panic_handler]
unsafe fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    sm::handle_panic(panic)
}
