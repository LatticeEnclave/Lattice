use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error occurs on parsering fdt: `{0}`")]
    Fdt(fdt::FdtError),
    #[error("Can't convert virtual address `{0:#x}` to physical address")]
    InvalidAddress(usize),
    #[error("{0}")]
    Other(&'static str),
}

impl Error {
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
