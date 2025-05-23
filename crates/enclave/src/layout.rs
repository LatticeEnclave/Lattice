use vm::{PAGE_SIZE, VirtMemArea, page_table::PTEFlags};

// use crate::DEFAULT_BOOTARG_ADDR;

// use crate::ecall::RT_VADDR_START;

pub const RT_VADDR_START: usize = 0xFFFF_FFFF_8000_0000;
pub const BIN_VADDR_START: usize = 0x20_0000_0000;
pub const DEFAULT_BOOTARG_ADDR: usize = 0xFFFF_FFFF_7FF0_0000;

pub struct Layout {
    pub rt: VirtMemArea,
    pub stack: VirtMemArea,
    pub binary: VirtMemArea,
    pub share: VirtMemArea,
    pub trampoline: VirtMemArea,
    pub bootargs: VirtMemArea,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            rt: VirtMemArea::default()
                .start(RT_VADDR_START)
                .flags(PTEFlags::rx().accessed()),
            stack: VirtMemArea::default()
                .start(RT_VADDR_START - PAGE_SIZE)
                .flags(PTEFlags::rw().dirty().accessed()),
            binary: VirtMemArea::default()
                .start(BIN_VADDR_START)
                .flags(PTEFlags::rw().dirty().accessed()),
            trampoline: VirtMemArea::default().flags(PTEFlags::rx().accessed()),
            share: VirtMemArea::default().flags(PTEFlags::rw().dirty().accessed()),
            bootargs: VirtMemArea::default()
                .start(DEFAULT_BOOTARG_ADDR)
                .flags(PTEFlags::rw().dirty().accessed()),
        }
    }
}
