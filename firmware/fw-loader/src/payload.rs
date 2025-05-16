use core::arch::global_asm;

global_asm!(include_str!(concat!(env!("OUT_DIR"), "/payload.S")));