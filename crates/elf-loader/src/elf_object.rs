extern crate alloc;
use core::iter::Filter;

use crate::elf_relocate::RelocationEntry;
use crate::error::ElfLoaderErr;
use crate::helper::ElfExt;
use crate::ElfLoader;
use xmas_elf::program::ProgramHeader::{self, Ph32, Ph64};
use xmas_elf::program::{ProgramIter, SegmentData, Type};
use xmas_elf::sections::SectionData;
use xmas_elf::symbol_table::{DynEntry64, Entry};
use xmas_elf::{header, ElfFile};

pub type LoadableHeaders<'a, 'b> = Filter<ProgramIter<'a, 'b>, fn(&ProgramHeader) -> bool>;

pub struct ElfObject<'a> {
    elf: ElfFile<'a>,
    //目前应该不需要这个成员
    // dyn_info: Option<DynamicInfo>,
    pub size: usize,
}

impl<'a> ElfObject<'a> {
    // pub fn new_at(addr: usize, )

    // Create a new ELFObnect
    pub fn new(data: &'a [u8]) -> Result<ElfObject<'a>, ElfLoaderErr> {
        let elf = ElfFile::new(data).map_err(|_| ElfLoaderErr::ElfParser {
            source: ("Not a ELF File"),
        })?;
        // log::debug!("elf info:  {:#x?}", elf.header.pt2);
        // let p= elf
        //     .program_iter()
        //     .find(|ph| ph.get_type() == Ok(Type::Dynamic));
        // let mut dyn_info: Option<DynamicInfo> = None;
        // match p {
        //     Some(dyn_p) => {
        //         dyn_info = DynamicInfo::
        //     }
        //     None => None
        // }
        let size = elf.load_segment_size();
        Ok(ElfObject { elf, size })
    }

    //动态链接需要解释器
    fn interpreter(&'a self) -> Result<&'a str, &'static str> {
        let header = self
            .elf
            .program_iter()
            .find(|ph| ph.get_type() == Ok(Type::Interp))
            .ok_or("no interp header")?;
        let data = match header.get_data(&self.elf)? {
            SegmentData::Undefined(data) => data,
            _ => return Err("cannot get the path"),
        };
        if data.len() < 2 {
            return Err("path string error");
        }
        //return the UTF-8 path string
        core::str::from_utf8(&data[..data.len() - 1]).map_err(|_| "from utf8 error")
    }

    pub fn get_symbol_address(&self, symbol: &str) -> Option<u64> {
        for section in self.elf.section_iter() {
            if let SectionData::SymbolTable64(entries) = section.get_data(&self.elf).unwrap() {
                for e in entries {
                    if e.get_name(&self.elf).unwrap() == symbol {
                        return Some(e.value());
                    }
                }
            }
        }
        None
    }

    //ELF文件的架构
    pub fn get_arch(&self) -> header::Machine {
        self.elf.header.pt2.machine().as_machine()
    }

    pub fn get_phnum(&self) -> usize {
        self.elf.header.pt2.ph_count() as usize
    }

    pub fn get_phdr(&self) -> usize {
        let phoff = self.elf.header.pt2.ph_offset() as usize;
        let phvaddr = self.elf
                                .program_iter()
                                .filter(|ph| ph.get_type().unwrap() == Type::Load && ph.offset() == 0)
                                .map(|ph| ph.virtual_addr() as usize)
                                .min()
                                .unwrap_or(0);
        
        phoff + phvaddr
    }

    //ELF程序入口偏移
    pub fn elf_entry(&self) -> usize {
        self.elf.header.pt2.entry_point() as usize
    }

    fn dynsym(&self) -> Result<&[DynEntry64], &'static str> {
        match self
            .elf
            .find_section_by_name(".dynsym")
            .ok_or(".dynsym not found")?
            .get_data(&self.elf)
            .map_err(|_| "corrupted .dynsym")?
        {
            SectionData::DynSymbolTable64(dsym) => Ok(dsym),
            _ => Err("bad .dynsym"),
        }
    }

    //ELF文件是否可以加载
    pub fn is_loadable(&self) -> Result<(), ElfLoaderErr> {
        let header = self.elf.header;
        let typ = header.pt2.type_().as_type();

        if header.pt1.version() != header::Version::Current {
            Err(ElfLoaderErr::UnsupportedElfVersion)
        } else if header.pt1.data() != header::Data::LittleEndian {
            Err(ElfLoaderErr::UnsupportedEndianness)
        } else if !(header.pt1.os_abi() == header::OsAbi::SystemV
            || header.pt1.os_abi() == header::OsAbi::Linux)
        {
            Err(ElfLoaderErr::UnsupportedAbi)
        } else if !(typ == header::Type::Executable || typ == header::Type::SharedObject) {
            Err(ElfLoaderErr::UnsupportedElfType)
        } else {
            Ok(())
        }
    }

    fn maybe_relocate<T: ElfLoader>(&self, load_region: &mut T) -> Result<(), ElfLoaderErr> {
        self.maybe_relocate_section(load_region, ".rela.dyn")?;
        self.maybe_relocate_section(load_region, ".rela.plt")?;

        Ok(())
    }

    fn maybe_relocate_section<T: ElfLoader>(
        &self,
        load_region: &mut T,
        section_name: &str,
    ) -> Result<(), ElfLoaderErr> {
        let data = self
            .elf
            .find_section_by_name(section_name)
            .ok_or(ElfLoaderErr::ElfParser {
                source: "cannot find section",
            })?
            .get_data(&self.elf)
            .map_err(|_| ElfLoaderErr::ElfParser {
                source: "corrupt section",
            })?;

        let entries = match data {
            SectionData::Rela64(entries) => entries,
            _ => return Err(ElfLoaderErr::ElfParser { source: "bad data" }),
        };

        let dynsym: &[DynEntry64] = self.dynsym()?;

        for entry in entries.iter() {
            // x86_64
            const REL_GOT: u32 = 6;
            const REL_PLT: u32 = 7;
            const REL_RELATIVE: u32 = 8;
            // riscv64
            const R_RISCV_64: u32 = 2;
            const R_RISCV_RELATIVE: u32 = 3;
            // aarch64
            const R_AARCH64_RELATIVE: u32 = 0x403;
            const R_AARCH64_GLOBAL_DATA: u32 = 0x401;

            match entry.get_type() {
                REL_GOT | REL_PLT | R_RISCV_64 | R_AARCH64_GLOBAL_DATA => {
                    let dynentry = &dynsym[entry.get_symbol_table_index() as usize];
                    // if dynentry.shndx() == 0 {
                    //     return Err("cannot find the table.");
                    // }
                    let symval = dynentry.value() as usize;
                    let addend = entry.get_addend() as usize;
                    let offset = entry.get_offset() as usize;

                    load_region.relocate(RelocationEntry {
                        offset: offset,
                        symval: Some(symval),
                        addend: addend,
                    })?;
                }
                REL_RELATIVE | R_RISCV_RELATIVE | R_AARCH64_RELATIVE => {
                    let offset = entry.get_offset() as usize;
                    let addend = entry.get_addend() as usize;

                    load_region.relocate(RelocationEntry {
                        offset: offset,
                        symval: None,
                        addend: addend,
                    })?;
                }
                _ => {
                    return Err(ElfLoaderErr::ElfParser {
                        source: "not implement yet",
                    })
                }
            }
        }
        Ok(())
    }

    pub fn load<T: ElfLoader>(&self, loader: &mut T) -> Result<(), ElfLoaderErr> {
        self.is_loadable()?;

        // 划分
        loader.map_program(self.iter_loadable_headers())?;

        for header in self.elf.program_iter() {
            let raw_data = match header {
                Ph32(inner) => inner.raw_data(&self.elf),
                Ph64(inner) => inner.raw_data(&self.elf),
            };
            let typ = header.get_type()?;
            match typ {
                Type::Load => {
                    loader.load(header.flags(), header.virtual_addr() as usize, raw_data)?;
                }
                Type::Tls => {
                    loader.tls(
                        header.virtual_addr() as usize,
                        header.file_size() as usize,
                        header.mem_size() as usize,
                        header.align() as usize,
                    )?;
                }
                _ => {}
            }
        }
        self.maybe_relocate(loader)?;

        let _ = self
            .elf
            .program_iter()
            .filter(|ph| ph.get_type().unwrap() == Type::GnuRelro)
            .map(|ph| loader.make_readonly(ph.virtual_addr() as usize, ph.mem_size() as usize));

        Ok(())
    }

    pub fn iter_loadable_headers(&self) -> LoadableHeaders {
        // Trying to determine loadeable headers
        fn select_load(pheader: &ProgramHeader) -> bool {
            match pheader {
                Ph32(header) => header
                    .get_type()
                    .map(|typ| typ == Type::Load)
                    .unwrap_or(false),
                Ph64(header) => header
                    .get_type()
                    .map(|typ| typ == Type::Load)
                    .unwrap_or(false),
            }
        }
        self.elf.program_iter().filter(select_load)
    }
}

fn program_is_loadable(ph: &ProgramHeader) -> bool {
    match ph {
        Ph32(header) => header
            .get_type()
            .map(|typ| typ == Type::Load)
            .unwrap_or(false),
        Ph64(header) => header
            .get_type()
            .map(|typ| typ == Type::Load)
            .unwrap_or(false),
    }
}
