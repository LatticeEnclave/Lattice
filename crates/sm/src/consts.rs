pub const GUARD_PAGE_SIZE: usize = 0x1000;
pub const MAX_HART_NUM: usize = 128;
//pub const PMP_NUM: usize = 16;

/// e_create flag:
/// - `a0`: memory vaddr start
/// - `a1`: memory size
pub const E_CREATE_FLAG: usize = 0xf0f0f0f0;
pub const E_DEBUG_FLAG: usize = 0xf0f0f0f1;

pub const FRAME_SIZE: usize = 0x1000;

pub const RT_INIT_ELF_START: usize = 0x1_0000_0000;

pub const RT_INIT_RT_START: usize = 0xFFFF_0000_0000;

pub const RT_VADDR_START: usize = 0xFFFF_FFFF_8000_0000;
pub const BIN_VADDR_START: usize = 0x20_0000_0000;

pub const BOOTARG_VADDR: usize = 0xFFFF_FFFF_7FF0_0000;
