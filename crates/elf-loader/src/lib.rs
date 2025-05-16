#![no_std]
#![feature(allocator_api)]
#![feature(type_alias_impl_trait)]
#![feature(error_in_core)]

mod dynamic;
mod elf_object;
mod elf_relocate;
mod error;
mod helper;
mod loader;
mod pm_loader;

pub use dynamic::DynamicInfo;
pub use elf_object::{ElfObject, LoadableHeaders};
pub use elf_relocate::RelocationEntry;
pub use error::ElfLoaderErr;
pub use helper::MapPermission;
pub use loader::ElfLoader;
pub use pm_loader::PhysMemLoader;

pub use xmas_elf::program::{Flags, ProgramHeader};

pub fn load_elf<'a>(
    elf: ElfObject<'_>,
    loader: &'a mut impl ElfLoader,
) -> Result<(), &'static str> {
    elf.load(loader).map_err(|_| "ELF load error")?;

    Ok(())
}
