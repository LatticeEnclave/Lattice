#![no_std]

pub use console::{_ln, _print, _println, Console};
pub use uart::init_console_uart;

pub mod console;
pub mod log_print;
mod uart;

pub mod log {
    pub use crate::{debug, error, info, trace, warn};
}
