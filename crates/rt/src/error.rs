use elf_loader::ElfLoaderErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    // #[error("ELF load error ocurred: {0}")]
    // ElfLoaderErr(ElfLoaderErr),

    #[error("{0}")]
    PageDirectoryFault(&'static str),

    #[error("{0}")]
    UndefinedSyscall(usize),

    #[error("{0}")]
    KernelInitErr(&'static str),

    #[error("[Runtime fault][{file}:{line}:{column}] {msg}")]
    RuntimeFault {
        file: &'static str,
        line: u32,
        column: u32,
        msg: &'static str,
    },
}

impl Error {
    pub fn page_directory_fault(msg: &'static str) -> Self {
        Self::PageDirectoryFault(msg)
    }

    pub fn kernel_init_err(msg: &'static str) -> Self {
        // Self::kernel_init_err(msg)
        todo!()
    }
}

#[macro_export]
macro_rules! runtime_fault {
    () => {
        $crate::Error::RuntimeFault {
            file: file!(),
            line: line!(),
            column: column!(),
            msg: "",
        }
    };
    ($msg:expr) => {
        $crate::Error::RuntimeFault {
            file: file!(),
            line: line!(),
            column: column!(),
            msg: $msg,
        }
    };
}
