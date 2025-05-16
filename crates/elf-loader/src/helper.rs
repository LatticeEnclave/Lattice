pub type PhysAddr = usize;

use xmas_elf::{program::Type, ElfFile};

pub trait ElfExt {
    fn load_segment_size(&self) -> usize;
}

impl ElfExt for ElfFile<'_> {
    fn load_segment_size(&self) -> usize {
        self.program_iter()
            .filter(|ph| ph.get_type().unwrap() == Type::Load)
            .map(|ph| (ph.virtual_addr() + ph.mem_size()) as usize)
            .max()
            .unwrap_or(0)
    }
}

use bitflags::bitflags;

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}
