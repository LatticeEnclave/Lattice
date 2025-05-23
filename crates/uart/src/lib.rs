#![no_std]

use core::fmt::Write;

const UART_THR_OFFSET: usize = 0x0;
const UART_LSR_OFFSET: usize = 0x5;

const UART_LSR_THRE: u32 = 0x20;

pub struct Reg {
    addr: *mut u8,
    width: usize,
}

impl Reg {
    #[inline(always)]
    pub fn new(addr: *mut u8, width: usize) -> Reg {
        Reg { addr, width }
    }

    #[inline(never)]
    pub fn write(&self, value: u32) {
        match self.width {
            1 => self.writeb(value as u8),
            2 => self.writew(value as u16),
            4 => self.writel(value),
            _ => panic!("unsupported width"),
        }
    }

    #[inline(always)]
    pub fn read(&self) -> u32 {
        match self.width {
            1 => self.readb() as u32,
            2 => self.readw() as u32,
            4 => self.readl(),
            _ => panic!("unsupported width"),
        }
    }

    #[inline(always)]
    pub fn contains(&self, mask: u32) -> bool {
        self.read() & mask == mask
    }

    #[inline(always)]
    fn readb(&self) -> u8 {
        unsafe { core::ptr::read_volatile(self.addr) }
    }

    #[inline(always)]
    fn readw(&self) -> u16 {
        unsafe { core::ptr::read_volatile(self.addr as *const u16) }
    }

    #[inline(always)]
    fn readl(&self) -> u32 {
        unsafe { core::ptr::read_volatile(self.addr as *const u32) }
    }

    #[inline(always)]
    fn writeb(&self, value: u8) {
        unsafe {
            core::ptr::write_volatile(self.addr, value);
        }
    }

    #[inline(always)]
    fn writew(&self, value: u16) {
        unsafe {
            core::ptr::write_volatile(self.addr as *mut u16, value);
        }
    }

    #[inline(always)]
    fn writel(&self, value: u32) {
        unsafe {
            core::ptr::write_volatile(self.addr as *mut u32, value);
        }
    }
}

pub struct MmioUart {
    base: *mut u8,
    reg_shift: usize,
    reg_width: usize,
}

unsafe impl Send for MmioUart {}

impl MmioUart {
    #[inline(always)]
    pub fn new(base: *mut u8, reg_shift: usize, reg_width: usize) -> MmioUart {
        MmioUart {
            base,
            reg_shift,
            reg_width,
        }
    }

    #[inline(always)]
    fn reg(&self, offset: usize) -> Reg {
        Reg::new(
            unsafe { self.base.add(offset << self.reg_shift) },
            self.reg_width,
        )
    }

    #[inline(always)]
    pub fn thr(&self) -> Reg {
        self.reg(UART_THR_OFFSET)
    }

    #[inline(always)]
    pub fn lsr(&self) -> Reg {
        self.reg(UART_LSR_OFFSET)
    }

    #[inline(always)]
    pub fn putc(&self, c: u8) {
        self.send(c as u32);
    }

    #[inline(always)]
    pub fn send(&self, data: u32) {
        let thr = self.thr();
        let lsr = self.lsr();
        while !lsr.contains(UART_LSR_THRE) {}
        thr.write(data);
    }
}

impl Write for MmioUart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            self.putc(c);
        }
        Ok(())
    }
}
