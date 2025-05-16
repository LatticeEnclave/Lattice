use core::mem::MaybeUninit;

use spin::Mutex;
use uart_16550::MmioSerialPort;

pub struct Console;
pub static UART: Mutex<MaybeUninit<MmioSerialPort>> = Mutex::new(MaybeUninit::uninit());

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe { UART.lock().assume_init_mut() }.send(c);
    }

    #[inline]
    fn put_str(&self, s: &str) {
        let mut uart = UART.lock();
        let uart = unsafe { uart.assume_init_mut() };
        for c in s.bytes() {
            uart.send(c);
        }
    }
}

// ----------------------

use core::ffi::c_char;

//pub fn init_log(uart: usize) {
//    //*UART.lock() = MaybeUninit::new(unsafe { MmioSerialPort::new(uart) });
//    rcore_console::init_console(&SbiConsole);
//    rcore_console::set_log_level(option_env!("LOG").or_else(|| Some("info")));
//}
//
pub fn init_log() {
    //*UART.lock() = MaybeUninit::new(unsafe { MmioSerialPort::new(uart) });
    rcore_console::init_console(&SbiConsole);
    rcore_console::set_log_level(option_env!("LOG").or_else(|| Some("info")));
}

//const SBI_NPUTS_ADDR: usize = usize_env_or!("SBI_NPUTS_ADDR", 0x0);
const SBI_NPUTS_ADDR: usize = 0x40006a92;

type SbiNputs = unsafe extern "C" fn(s: *const c_char, len: usize) -> usize;

pub struct SbiConsole;
impl rcore_console::Console for SbiConsole {
    #[inline]
    fn put_char(&self, c: u8) {
        let sbi_nputs: SbiNputs = unsafe { core::mem::transmute(SBI_NPUTS_ADDR) };
        unsafe {
            sbi_nputs(&c as *const u8 as *const c_char, 1);
        }
    }

    #[inline]
    fn put_str(&self, s: &str) {
        let sbi_nputs: SbiNputs = unsafe { core::mem::transmute(SBI_NPUTS_ADDR) };
        unsafe {
            sbi_nputs(s.as_ptr() as *const c_char, s.len());
        }
    }
}
