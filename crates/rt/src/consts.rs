use crate::scratch::ScratchManager;
use core::mem::size_of;

macro_rules! align {
    ($v:expr, $t:expr) => {
        $v & !($t - 1)
    };
}

#[allow(unused)]
pub const PAGE_SIZE: usize = 0x1000;

/// Runtime base address
pub const RUNTIME_VA_START: usize = 0xffffffff80000000;

/// Kernel information space. One page before Runtime base address
pub const LUE_KERNEL_VADDR: usize = 0xffffffff90000000;

/// Kernel information space. One page before Runtime base address
pub const LDE_KERNEL_VADDR: usize = 0xffffffff70000000;

/// Scratch space
pub const STACK_SIZE: usize = 0x20000;
pub const LUE_SCRATCH_START_VADDR: usize = align!(LUE_KERNEL_VADDR - size_of::<ScratchManager>(), PAGE_SIZE);
pub const LDE_SCRATCH_START_VADDR: usize = align!(LDE_KERNEL_VADDR - size_of::<ScratchManager>(), PAGE_SIZE);


/// Heap space
pub const HEAP_SIZE: usize = 0x20000;
pub const LUE_HEAP_START: usize = align!(LUE_SCRATCH_START_VADDR - HEAP_SIZE, PAGE_SIZE);
pub const LDE_HEAP_START: usize = align!(LDE_SCRATCH_START_VADDR - HEAP_SIZE, PAGE_SIZE);

/// pub const PAGE_SIZE_BITS: usize = 12;
pub const LUE_ELF_LOAD_OFFSET: usize = LUE_KERNEL_VADDR + PAGE_SIZE;
pub const LDE_ELF_LOAD_OFFSET: usize = LDE_KERNEL_VADDR + PAGE_SIZE;

/// pre-defined max hart numbers
pub const MAX_HART_NUM: usize = 8;

/// task stack position
pub const TASK_STACK_TOP: usize = 0x40000000;


/// user space mapping start address
pub const USER_BUFFER: usize = 0xffffffff90001000;

/// user dellocate addr
pub const USER_CLEAN_BUFFER: usize = 0xffffffff900fc000;

/// buffer limitation
pub const MAX_BUFFER_SIZE: usize = 0x7ffff000;

/// arbitrary VA to start looking for large mappings
pub const USR_ANON_REGION_START: usize = 0x2000000000;

/// shared memory size
pub const SHARED_MEMORY_REGION_SIZE: usize = 0x1000;

/// driver stack size
pub const DRIVER_STACK_SIZE: usize = 2;