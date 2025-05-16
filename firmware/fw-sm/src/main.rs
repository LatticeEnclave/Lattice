#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(fn_align)]

extern crate htee_channel;
extern crate htee_console;
extern crate htee_device;
extern crate htee_macro;
extern crate riscv;
extern crate sbi;
extern crate sm;
extern crate spin;
extern crate pmp;

mod init;
mod trap;

use core::arch::{asm, global_asm, naked_asm};

use htee_console::log;

// #[naked]
// #[no_mangle]
// #[link_section = ".text.entry"]
// unsafe extern "C" fn _start(next_addr: usize, arg1: usize) -> ! {
//     naked_asm!(
//         // use sbi stack
//         // "csrr sp, mscratch",
//         // entry and trap vector
//         "j {init_sm}",
//         init_sm = sym init::sm_init,
//     )
// }

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(next_addr: usize, arg1: usize) -> ! {
    naked_asm!(
        // use sbi stack
        "csrr sp, mscratch",
        // entry and trap vector
        "j {init_sm}",
        init_sm = sym init::sm_init,
    )
}

/// Trap vector
#[naked]
#[no_mangle]
#[repr(align(0x10))]
unsafe extern "C" fn trap_vec() {
    naked_asm!(
        ".align 4",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        "j {default}",
        default = sym trap_vec,
    )
}

#[panic_handler]
unsafe fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    log::error!("{}", _panic);

    loop {
        asm!("wfi")
    }
}

// global_asm!(include_str!(concat!(env!("OUT_DIR"), "/sbi.S")));