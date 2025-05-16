pub mod client;
pub mod server;

#[macro_export]
macro_rules! arg_offset_of {
    ($reg: ident) => {
        core::mem::offset_of!(htee_vstack::ArgRegs, $reg) as isize
    };
}

#[macro_export]
macro_rules! proxy_client_ecall {
    ($val: ident) => {
        core::arch::asm!(
            "ld a0, {}(t0)",
            "ld a1, {}(t0)",
            "ld a2, {}(t0)",
            "ld a3, {}(t0)",
            "ld a4, {}(t0)",
            "ld a5, {}(t0)",
            "ld a6, {}(t0)",
            "ld a7, {}(t0)",
            "ecall",
            "sd a0, {}(t0)",
            "sd a1, {}(t0)",
            "sd a2, {}(t0)",
            "sd a3, {}(t0)",
            "sd a4, {}(t0)",
            "sd a5, {}(t0)",
            "sd a6, {}(t0)",
            "sd a7, {}(t0)",
            const arg_offset_of!(a0),
            const arg_offset_of!(a1),
            const arg_offset_of!(a2),
            const arg_offset_of!(a3),
            const arg_offset_of!(a4),
            const arg_offset_of!(a5),
            const arg_offset_of!(a6),
            const arg_offset_of!(a7),
            const arg_offset_of!(a0),
            const arg_offset_of!(a1),
            const arg_offset_of!(a2),
            const arg_offset_of!(a3),
            const arg_offset_of!(a4),
            const arg_offset_of!(a5),
            const arg_offset_of!(a6),
            const arg_offset_of!(a7),
            in("t0") $val
        )
    };
}

#[macro_export]
macro_rules! proxy_load_args {
    ($val: ident) => {
        asm!(
            "ld a0, {}(t0)",
            "ld a1, {}(t0)",
            "ld a2, {}(t0)",
            "ld a3, {}(t0)",
            "ld a4, {}(t0)",
            "ld a5, {}(t0)",
            "ld a6, {}(t0)",
            "ld a7, {}(t0)",
            const arg_offset_of!(a0),
            const arg_offset_of!(a1),
            const arg_offset_of!(a2),
            const arg_offset_of!(a3),
            const arg_offset_of!(a4),
            const arg_offset_of!(a5),
            const arg_offset_of!(a6),
            const arg_offset_of!(a7),
            in("t0") $val
        )
    };
}

#[macro_export]
macro_rules! proxy_save_args {
    ($val: ident) => {
        asm!(
            "sd a0, {}(t0)",
            "sd a1, {}(t0)",
            "sd a2, {}(t0)",
            "sd a3, {}(t0)",
            "sd a4, {}(t0)",
            "sd a5, {}(t0)",
            "sd a6, {}(t0)",
            "sd a7, {}(t0)",
            const arg_offset_of!(a0),
            const arg_offset_of!(a1),
            const arg_offset_of!(a2),
            const arg_offset_of!(a3),
            const arg_offset_of!(a4),
            const arg_offset_of!(a5),
            const arg_offset_of!(a6),
            const arg_offset_of!(a7),
            in("t0") $val
        )
    };
}

#[inline(never)]
pub unsafe fn proxy_system_call(start: usize) {
    proxy_client_ecall!(start);
}
