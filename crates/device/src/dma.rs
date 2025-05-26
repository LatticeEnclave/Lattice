use bitflags::bitflags;
use fdt::Fdt;

use crate::error::Error;

mod axi;

pub fn find_dma(fdt: &Fdt<'_>) -> Result<*mut u8, Error> {
    let dma_node = fdt
        .find_node("/soc/dma-controller")
        .ok_or(Error::NodeNotFound("dma-controller"))?;
    let reg = dma_node
        .reg()
        .and_then(|mut regs| regs.next())
        .ok_or(Error::RegNotFound("dma-controller"))?;
    let addr = reg.starting_address as *mut u8;
    // let size = reg.size.unwrap_or(0);
    Ok(addr)
}

#[inline]
pub fn in_region(addr: usize, start: usize) -> bool {
    addr >= start && addr < (start + 0x1000)
}

bitflags! {
    pub struct CSRFlags: usize {
        const DMA_ENABLE = 0b1;
    }
}
