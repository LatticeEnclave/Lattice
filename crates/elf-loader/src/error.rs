#[derive(PartialEq, Clone, Debug)]
pub enum ElfLoaderErr {
    ElfParser { source: &'static str },
    OutOfMemory,
    SymbolTableNotFound,
    UnsupportedElfFormat,
    UnsupportedElfVersion,
    UnsupportedEndianness,
    UnsupportedAbi,
    UnsupportedElfType,
    UnsupportedSectionData,
    UnsupportedArchitecture,
    UnsupportedRelocationEntry,
}

impl From<&'static str> for ElfLoaderErr {
    fn from(source: &'static str) -> Self {
        ElfLoaderErr::ElfParser { source }
    }
}

// impl core::fmt::Display for ElfLoaderErr {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         match self {
//             ElfLoaderErr::ElfParser { source } => write!(f, "Error in ELF parser: {}", source),
//             ElfLoaderErr::OutOfMemory => write!(f, "Out of memory"),
//             ElfLoaderErr::SymbolTableNotFound => write!(f, "No symbol table in the ELF file"),
//             ElfLoaderErr::UnsupportedElfFormat => write!(f, "ELF format not supported"),
//             ElfLoaderErr::UnsupportedElfVersion => write!(f, "ELF version not supported"),
//             ElfLoaderErr::UnsupportedEndianness => write!(f, "ELF endianness not supported"),
//             ElfLoaderErr::UnsupportedAbi => write!(f, "ELF ABI not supported"),
//             ElfLoaderErr::UnsupportedElfType => write!(f, "ELF type not supported"),
//             ElfLoaderErr::UnsupportedSectionData => write!(f, "Can't handle this section data"),
//             ElfLoaderErr::UnsupportedArchitecture => write!(f, "Unsupported Architecture"),
//             ElfLoaderErr::UnsupportedRelocationEntry => {
//                 write!(f, "Can't handle relocation entry")
//             }
//         }
//     }
// }

// impl core::error::Error for ElfLoaderErr {
//     fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
//         None
//     }
// }
