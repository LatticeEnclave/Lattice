#[cfg(feature = "debug")]
pub const LEVEL: LogLevel = LogLevel::Debug;

#[cfg(feature = "trace")]
pub const LEVEL: LogLevel = LogLevel::Trace;

#[cfg(feature = "info")]
pub const LEVEL: LogLevel = LogLevel::Info;

#[cfg(feature = "error")]
pub const LEVEL: LogLevel = LogLevel::Error;

#[cfg(not(any(
    feature = "info",
    feature = "debug",
    feature = "trace",
    feature = "error"
)))]
pub const LEVEL: LogLevel = LogLevel::Info;

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

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        if $crate::log_print::LogLevel::Info <= $crate::log_print::LEVEL {
            $crate::println!("[INFO] {}", core::format_args!($($arg)*));

        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log_print::LogLevel::Debug <= $crate::log_print::LEVEL {
            $crate::println!("[Debug] {}", core::format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        if $crate::log_print::LogLevel::Error <= $crate::log_print::LEVEL {
            $crate::println!("[Error] {}", core::format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::log_print::LogLevel::Trace <= $crate::log_print::LEVEL {
            $crate::println!("[Trace] {}", core::format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! warn{
    ($($arg:tt)*) => {
        if $crate::log_print::LogLevel::Warn <= $crate::log_print::LEVEL {
            $crate::print!("[Warning] ");
            $crate::println!($($arg)*);
        }
    };
}
