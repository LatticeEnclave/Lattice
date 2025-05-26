use crate::console::{Console, init_console};
use core::mem::MaybeUninit;
use device::console::Uart;
use spin::mutex::Mutex;
use uart::MmioUart;

static UART: Mutex<MaybeUninit<MmioUart>> = Mutex::new(MaybeUninit::uninit());
// static UART: Once<Mutex<MmioUart>> = Once::new();
// static mut UART: MaybeUninit<Mutex<MmioUart>> = MaybeUninit::uninit();

struct Device;

#[allow(static_mut_refs)]
impl Console for Device {
    #[inline]
    fn put_char(&self, c: u8) {
        let mut lock = UART.lock();
        let uart = unsafe { lock.assume_init_mut() };
        // let uart = unsafe { uart.assume_init_mut() };
        uart.putc(c);
    }

    #[inline]
    fn put_str(&self, s: &str) {
        let mut lock = UART.lock();
        let uart = unsafe { lock.assume_init_mut() };

        // let uart = unsafe { uart.assume_init_ref() };
        for c in s.bytes() {
            if c == b'\n' {
                uart.putc(b'\r');
            }
            uart.putc(c);
        }
    }
}

pub fn init_console_uart(uart: Uart) {
    *UART.lock() = MaybeUninit::new(MmioUart::new(uart.addr, uart.reg_shift, uart.reg_io_width));

    // We don't need to init the UART here, because it's already done in the opensbi.

    init_console(&Device);
}
