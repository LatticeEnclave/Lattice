use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Physical address `{addr:#x}` is out of memory")]
    OoM { addr: usize },
    #[error("Error occurs on parsering fdt: `{0}`")]
    Fdt(fdt::FdtError),
    #[error("Can't convert virtual address `{0:#x}` to physical address")]
    InvalidAddress(usize),
    #[error("Invalid memory access at {pc:#x}")]
    InvalidMemoryAccess { pc: usize },
    #[error("Failed to update pmp registers at {mepc:#x} with mtval {mtval:#x}")]
    PMPUpdateFailed { mepc: usize, mtval: usize },
    #[error("Page fault at {addr:#x}")]
    PageFault { addr: usize },
    #[error("Unknown instruction extension at {0:#x}")]
    UnknownInstrExt(usize),
    #[error("Invalid enclave type")]
    InvalidEnclaveType,
    #[error("Invalid enclave id: {0:#x}")]
    InvalidEnclaveId(usize),
    #[error("Invalid lde")]
    InvalidLde,
    #[error("Channel full")]
    ChannelFull,
    #[error("{0}")]
    Other(&'static str),
}

impl Error {
    pub fn oom(addr: usize) -> Self {
        Self::OoM { addr }
    }

    /// Raise `InvalidMemoryAccess` at `pc`
    pub fn ima(pc: usize) -> Self {
        Self::InvalidMemoryAccess { pc }
    }

    pub fn pf(addr: usize) -> Self {
        Self::PageFault { addr }
    }

    /// Raise `TranslateFailed`
    pub fn tf(vaddr: usize) -> Self {
        Self::InvalidAddress(vaddr)
    }

    pub fn other(msg: &'static str) -> Self {
        Self::Other(msg)
    }

    pub fn fdt_node_not_found() -> Self {
        Self::other("fdt node not found")
    }

    pub fn fdt_prop_not_found() -> Self {
        Self::other("fdt prop not found")
    }

    pub fn fdt_value_parser_err() -> Self {
        Self::other("fdt value parser error")
    }
}

impl From<fdt::FdtError> for Error {
    fn from(value: fdt::FdtError) -> Self {
        Self::Fdt(value)
    }
}

impl<'a> From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::Other(value)
    }
}
