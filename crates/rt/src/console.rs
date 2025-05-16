use core::fmt::{self, Write};

use crate::kernel::LinuxUserKernel;
use spin::Mutex;
use uart::MmioUart;

pub struct Console {
    pub uart: Mutex<MmioUart>,
}

impl Console {
    pub fn new(uart: MmioUart) -> Console {
        Console {
            uart: Mutex::new(uart),
        }
    }

    #[inline(always)]
    pub fn put_char(&self, c: u8) {
        let uart = self.uart.lock();
        uart.putc(c);
    }

    #[inline(always)]
    pub fn put_str(&self, s: &str) {
        let uart = self.uart.lock();
        for c in s.bytes() {
            if c == b'\n' {
                uart.putc(b'\r');
            }
            uart.putc(c);
        }
    }
}

struct Logger;

impl Write for Logger {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let console = unsafe { LinuxUserKernel::from_sscratch().get_console() };
        console.put_str(s);
        Ok(())
    }
}

#[doc(hidden)]
#[inline]
pub fn _print(args: fmt::Arguments) {
    Logger.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(core::format_args!($($arg)*));
    }
}

/// 格式化打印并换行。
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        $crate::_print(core::format_args!($($arg)*));
        $crate::_print(core::format_args!("\n"));
    }}
}

#[derive(Clone, Copy)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl PartialEq for LogLevel {
    fn eq(&self, other: &Self) -> bool {
        (*self as usize) == (*other as usize)
    }
}

impl PartialOrd for LogLevel {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        (*self as usize).partial_cmp(&(*other as usize))
    }
}

#[cfg(not(debug_assertions))]
pub const LEVEL: LogLevel = LogLevel::Info;

#[cfg(debug_assertions)]
pub const LEVEL: LogLevel = LogLevel::Debug;

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        if $crate::log::LogLevel::Info <= $crate::log::LEVEL {
            $crate::print!("[INFO] ");
            $crate::println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::console::LogLevel::Debug <= $crate::console::LEVEL {
            $crate::print!("[Debug] ");
            $crate::println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        if $crate::console::LogLevel::Error <= $crate::console::LEVEL {
            $crate::print!("[Error] ");
            $crate::println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::console::LogLevel::Trace <= $crate::console::LEVEL {
            $crate::print!("[Trace] ");
            $crate::println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! warn{
    ($($arg:tt)*) => {
        if $crate::console::LogLevel::Warn <= $crate::console::LEVEL {
            $crate::print!("[Warning] ");
            $crate::println!($($arg)*);
        }
    };
}
