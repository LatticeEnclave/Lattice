#[repr(C)]
pub struct Scratch {
    /// Start (or base) address of firmware linked to OpenSBI library
    pub fw_start: usize,
    /// Size (in bytes) of firmware linked to OpenSBI library
    pub fw_size: usize,
    /// Offset (in bytes) of the R/W section
    pub fw_rw_offset: usize,
    /// Offset (in bytes) of the heap area
    pub fw_heap_offset: usize,
    /// Size (in bytes) of the heap area
    pub fw_heap_size: usize,
    /// Arg1 (or 'a1' register) of next booting stage for this HART
    pub next_arg1: usize,
    /// Address of next booting stage for this HART
    pub next_addr: usize,
    /// Privilege mode of next booting stage for this HART
    pub next_mode: usize,
    /** Warm boot entry point address for this HART */
    pub warmboot_addr: usize,
    /** Address of sbi_platform */
    pub platform_addr: usize,
    /** Address of HART ID to sbi_scratch conversion function */
    pub hartid_to_scratch: usize,
    /** Address of trap exit function */
    pub trap_exit: usize,
    /** Temporary storage */
    pub tmp0: usize,
    /** Options for OpenSBI library */
    pub options: usize,
}