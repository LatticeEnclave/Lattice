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

pub fn init_log(uart: usize) {
    *UART.lock() = MaybeUninit::new(unsafe { MmioSerialPort::new(uart) });
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG").or_else(|| Some("info")));
}
