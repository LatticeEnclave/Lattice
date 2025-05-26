/// Copy from rcore-console with some modification.
use core::fmt::{self, Write};
use spin::Once;

/// 这个接口定义了向控制台“输出”这件事。
pub trait Console: Sync {
    /// 向控制台放置一个字符。
    fn put_char(&self, c: u8);

    /// 向控制台放置一个字符串。
    ///
    /// 如果使用了锁，覆盖这个实现以免反复获取和释放锁。
    #[inline]
    fn put_str(&self, s: &str) {
        for c in s.bytes() {
            self.put_char(c);
        }
    }
}

pub fn console_is_some() -> bool {
    CONSOLE.get().is_some()
}

/// 库找到输出的方法：保存一个对象引用，这是一种单例。
static CONSOLE: Once<&'static dyn Console> = Once::new();

/// 用户调用这个函数设置输出的方法。
#[inline]
pub fn init_console(console: &'static dyn Console) {
    CONSOLE.call_once(|| console);
    //log::set_logger(&Printer).unwrap();
    //log::set_max_level(
    //    option_env!("LOG")
    //        .map(|level| match level {
    //            "debug" | "DEBUG" => log::LevelFilter::Debug,
    //            "info" | "INFO" => log::LevelFilter::Info,
    //            "trace" | "TRACE" => log::LevelFilter::Trace,
    //            _ => log::LevelFilter::Info,
    //        })
    //        .unwrap_or(log::LevelFilter::Info),
    //);
}

/// 打印。
///
/// 给宏用的，用户不会直接调它。
#[doc(hidden)]
#[inline]
pub fn _print(args: fmt::Arguments) {
    Printer.write_fmt(args).unwrap();
}

#[doc(hidden)]
#[inline]
pub fn _println(args: fmt::Arguments) {
    Printer.write_fmt(core::format_args!("{}\n", args)).unwrap();
}

#[doc(hidden)]
#[inline]
pub fn _ln() {
    Printer.write_str("\n").unwrap()
}

/// 格式化打印。
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
        $crate::_println(core::format_args!($($arg)*));
    }}
}

/// 这个 Unit struct 是 `core::fmt` 要求的。
struct Printer;

/// 实现 [`Write`] trait，格式化的基础。
impl Write for Printer {
    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        let c = CONSOLE.get().unwrap();
        c.put_str(s);
        Ok(())
    }
}
