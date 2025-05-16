use core::alloc::Allocator;

use crate::elf_relocate::RelocationEntry;
use crate::helper::MapPermission;
use crate::ElfLoader;
use crate::ElfLoaderErr;
use crate::elf_object::LoadableHeaders;
use xmas_elf::header;
use xmas_elf::program::{Flags, ProgramHeader};

pub struct ElfArea {
    pub entry: usize,
    pub length: usize,
    pub permission: MapPermission,
}

impl ElfArea {
    pub fn new(entry: usize, length: usize, permission: MapPermission) -> ElfArea {
        ElfArea {
            entry,
            length,
            permission,
        }
    }
}

/// 加载ELF文件到物理地址中
pub struct PhysMemLoader<A: Allocator> {
    pub base: usize,
    pub size: usize,
    pub entry: Option<usize>,
    /// The memory allocator
    pub heap_alloc: A,
}

impl<A: Allocator> PhysMemLoader<A> {
    pub fn new(base: usize, size: usize, heap_alloc: A) -> PhysMemLoader<A> {
        PhysMemLoader {
            base,
            size,
            entry: None,
            heap_alloc,
        }
    }

    pub fn get_base(&self) -> usize {
        self.base
    }
}

impl<A: Allocator> ElfLoader for PhysMemLoader<A> {
    fn map_program(&mut self, load_headers: LoadableHeaders) -> Result<(), ElfLoaderErr> {
        // if self.size < (ph.virtual_addr() + ph.mem_size()) as usize {
        //     return Err(ElfLoaderErr::OutOfMemory);
        // }
        // let entry = self.base + ph.virtual_addr() as usize;
        // let length = ph.mem_size() as usize;
        // let ph_flags = ph.flags();
        // let mut map_perm = MapPermission::U;
        // if ph_flags.is_read() {
        //     map_perm |= MapPermission::R;
        // }
        // if ph_flags.is_write() {
        //     map_perm |= MapPermission::W;
        // }
        // if ph_flags.is_execute() {
        //     map_perm |= MapPermission::X;
        // }
        todo!()
    }

    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), &'static str> {
        let rel_addr = self.base + entry.offset;
        let rel_value = self.base + entry.symval.unwrap_or(0) + entry.addend;
        unsafe {
            let dst = rel_addr as *mut usize;
            *dst = rel_value;
        }
        Ok(())
    }

    fn load(&mut self, flags: Flags, entry: usize, region: &[u8]) -> Result<(), ElfLoaderErr> {
        let dst_addr = self.base + entry;
        unsafe {
            let dst = core::slice::from_raw_parts_mut(dst_addr as *mut u8, region.len());
            dst.copy_from_slice(region);
        }
        Ok(())
    }

    fn tls(
        &mut self,
        _tdata_start: usize,
        _tdata_length: usize,
        _total_size: usize,
        _align: usize,
    ) -> Result<(), ElfLoaderErr> {
        Ok(())
    }
}
