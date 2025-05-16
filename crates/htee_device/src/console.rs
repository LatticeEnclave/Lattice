use core::ops::Range;

use fdt::Fdt;

use crate::error::Error;

const DEFAULT_UART_REG_SHIFT: usize = 0;
const DEFAULT_UART_FREQ: usize = 0;
const DEFAULT_UART_BAUD: usize = 115200;
const DEFAULT_UART_REG_IO_WIDTH: usize = 1;
const DEFAULT_UART_REG_OFFSET: usize = 0;

pub struct Console;

#[derive(Debug, Clone)]
pub struct Uart {
    pub addr: *mut u8,
    pub size: usize,
    pub freq: usize,
    pub baud: usize,
    pub reg_shift: usize,
    pub reg_io_width: usize,
    pub reg_offset: usize,
}

impl Uart {
    pub fn get_reg(&self) -> Range<usize> {
        (self.addr as usize)..(self.addr as usize + self.size)
    }
}

pub fn find_uart(fdt: &Fdt<'_>) -> Result<Uart, Error> {
    let chosen = fdt.chosen();
    let stdout = chosen.stdout().ok_or(Error::NodeNotFound("stdout"))?;
    let uart_node = stdout.node();
    let reg = uart_node
        .reg()
        .and_then(|mut regs| regs.next())
        .ok_or(Error::RegNotFound("stdout"))?;
    let freq = uart_node
        .property("clock-frequency")
        .and_then(|prop| prop.as_usize())
        .unwrap_or(DEFAULT_UART_FREQ);
    let baud = uart_node
        .property("current-speed")
        .and_then(|prop| prop.as_usize())
        .unwrap_or(DEFAULT_UART_BAUD);
    let reg_shift = uart_node
        .property("reg-shift")
        .and_then(|prop| prop.as_usize())
        .unwrap_or(DEFAULT_UART_REG_SHIFT);
    let reg_io_width = uart_node
        .property("reg-io-width")
        .and_then(|prop| prop.as_usize())
        .unwrap_or(DEFAULT_UART_REG_IO_WIDTH);
    let reg_offset = uart_node
        .property("reg-offset")
        .and_then(|prop| prop.as_usize())
        .unwrap_or(DEFAULT_UART_REG_OFFSET);

    Ok(Uart {
        addr: reg.starting_address as usize as *mut u8,
        size: reg.size.unwrap_or(0),
        freq,
        baud,
        reg_shift,
        reg_io_width,
        reg_offset,
    })
}
