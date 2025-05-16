use xmas_elf::program::{Flags, ProgramHeader};

use crate::elf_relocate::RelocationEntry;
use crate::error::ElfLoaderErr;
use crate::elf_object::LoadableHeaders;

pub trait ElfLoader {
    //根据"load_header"分配虚拟内存空间
    fn map_program(&mut self, load_headers: LoadableHeaders) -> Result<(), ElfLoaderErr>;

    //将region中的数据加载到以base开头的虚拟内存空间
    fn load(&mut self, flags: Flags, entry: usize, region: &[u8]) -> Result<(), ElfLoaderErr>;

    //执行重定位
    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), &'static str>;

    //初始TLS数据存放地址
    fn tls(
        &mut self,
        _tdata_start: usize,
        _tdata_length: usize,
        _total_size: usize,
        _align: usize,
    ) -> Result<(), ElfLoaderErr> {
        Ok(())
    }

    fn make_readonly(&mut self, _base: usize, _size: usize) -> Result<(), ElfLoaderErr> {
        Ok(())
    }
}



