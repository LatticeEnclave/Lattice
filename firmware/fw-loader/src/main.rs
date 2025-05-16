#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::arch::{asm, naked_asm};
//use htee_console::{init_console_uart, log, println};
//use htee_device::device::DeviceInfo;
use htee_macro::usize_env_or;
use riscv::register::{mepc, mhartid, mstatus};

use crate::reloc::{reloc_payload, reloc_sm};

mod payload;
mod reloc;

const PAYLOAD_ADDR: usize = usize_env_or!("FW_TEXT_START", 0x8000_0000) + 0x20_0000;
const SM_ADDR: usize =
    usize_env_or!("FW_TEXT_START", 0x8000_0000) + usize_env_or!("SBI_SIZE", 0x60000);

#[naked]
#[no_mangle]
#[link_section = ".start"]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "
            j {init}
        ",
        init = sym init,
    )
}

#[naked]
#[no_mangle]
unsafe extern "C" fn init(next_addr: usize, fdt: usize) -> ! {
    naked_asm!(
        "
            la gp, _global_pointer
            la sp, _loader_sp
            j {entry}
        ",
        entry = sym entry,
    );
}

/// reloc payload and sm, then call sm_init
#[no_mangle]
fn entry(next_addr: usize, fdt: usize) -> ! {
    //let device = DeviceInfo::new(fdt as *const u8).unwrap();
    //let uart = device.get_uart().unwrap();
    //init_console_uart(uart);

    //println!("Console initialized");
    //log::info!("Console initialized");

    let (sm_start, sm_end, payload_start, payload_end) = binary_addrs();

    reloc_sm(sm_start, SM_ADDR, sm_end);
    reloc_payload(payload_start, next_addr, payload_end);

    call_sm_init(SM_ADDR, next_addr, fdt);
}

fn call_sm_init(sm_addr: usize, next_addr: usize, fdt: usize) -> ! {
    let sm_init_fn: fn(usize, usize) -> ! = unsafe { core::mem::transmute(sm_addr as *const ()) };
    sm_init_fn(next_addr, fdt)
}

fn binary_addrs() -> (usize, usize, usize, usize) {
    let mut payload_start: usize;
    let mut payload_end: usize;
    let mut sm_start: usize;
    let mut sm_end: usize;

    unsafe {
        asm!(
            "la   {payload_start}, _payload_start",
            "la   {payload_end}, _payload_end",
            "la   {sm_start}, _sm_start",
            "la   {sm_end}, _sm_end",
            payload_start = out(reg) payload_start,
            payload_end = out(reg) payload_end,
            sm_start = out(reg) sm_start,
            sm_end = out(reg) sm_end,
        )
    }

    (sm_start, sm_end, payload_start, payload_end)
}

#[panic_handler]
unsafe fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        asm!("ebreak", "wfi", options(noreturn));
    }
}
