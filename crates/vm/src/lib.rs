#![no_std]
#![feature(step_trait)]

pub mod allocator;
pub mod consts;
pub mod mm;
pub mod page_table;
pub mod pm;
mod translate;
pub mod vm;

pub mod prelude {
    pub use crate::{align_down, align_up};
    pub use crate::{BarePtReader, PhysAddr, PhysPageNum, Translate, VirtAddr, VirtPageNum};
}

pub use consts::*;
pub use page_table::BarePtReader;
pub use page_table::{PageTableReader, PageTableWriter};
pub use pm::{PhysAddr, PhysPageNum};
use riscv::register::satp;
pub use translate::{Translate, VAddrTranslator};
pub use vm::{VirtAddr, VirtPageNum};

#[macro_export]
macro_rules! align_up {
    ($addr:expr, $align:expr) => {
        ($addr + $align - 1) & !($align - 1)
    };
}

#[macro_export]
macro_rules! align_down {
    ($addr:expr, $align:expr) => {
        $addr & !($align - 1)
    };
}

#[macro_export]
macro_rules! aligned {
    ($vaddr:expr, $align:expr) => {
        $vaddr & !($align - 1) == $vaddr
    };
}

pub fn trans_direct(
    vaddr: impl Into<VirtAddr>,
    pt: impl Into<PhysPageNum>,
    mode: satp::Mode,
) -> Option<PhysAddr> {
    let vaddr: VirtAddr = vaddr.into();
    let pt: PhysPageNum = pt.into();
    let paddr = vaddr.translate(pt, mode, &BarePtReader)?;
    Some(paddr.into())
}
