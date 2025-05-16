use core::fmt::Display;

pub use elf::Sections;

#[repr(C, align(0x1000))]
pub struct LseInfo {
    pub mem: MemInfo,
    pub rt: RtInfo,
}

impl Display for LseInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "
mem_start:\t{:#x}
mem_size:\t{:#x}
rt_start:\t{:#x}
",
            self.mem.start as usize,
            self.mem.page_num * 0x1000 as usize,
            self.rt.ptr as usize,
        ))?;

        Ok(())
    }
}

#[repr(C, align(0x1000))]
pub struct LueInfo {
    pub mem: MemInfo,
    pub bin: BinInfo,
    pub rt: RtInfo,
    // pub mods: &'a [ModInfo],
    pub shared: SharedInfo,
    pub unused: UnusedInfo,
}

impl Display for LueInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "
mem_start:\t{:#x}
mem_size:\t{:#x}
elf_start:\t{:#x}
rt_start:\t{:#x}
shared_start:\t{:#x}
unused_start:\t{:#x}
",
            self.mem.start as usize,
            self.mem.page_num * 0x1000 as usize,
            self.bin.ptr as usize,
            self.rt.ptr as usize,
            self.shared.ptr as usize,
            self.unused.start as usize
        ))?;

        Ok(())
    }
}

pub struct UnusedInfo {
    pub start: *const u8,
    pub size: usize,
}

pub struct MemInfo {
    pub start: *const u8,
    pub page_num: usize,
}

pub struct BinInfo {
    pub ptr: *const u8,
    pub size: usize,
}

pub struct RtInfo {
    pub ptr: *const u8,
    pub size: usize,
}

/// Including
pub struct ModMemInfo {
    pub ptr: *const u8,
    pub size: usize,
}

pub struct ModInfo {
    /// ptr to the module binary header
    pub ptr: *const u8,
    pub size: usize,
    pub args: Option<ModArgInfo>,
}

#[derive(Debug)]
pub struct ModArgInfo {
    pub ptr: *const u8,
    pub len: usize,
}

impl ModArgInfo {
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_raw_parts(self.ptr, self.len) }
    }
}

pub struct SharedInfo {
    pub ptr: *const u8,
    pub size: usize,
}

pub struct DriverInfo {
    pub ptr: *const u8,
    pub size: usize,
    pub sections: elf::Sections,
    pub name: [u8; 64],
}

pub struct LdeInfo {
    pub mem: MemInfo,
    pub bin: BinInfo,
    pub rt: RtInfo,
    // pub shared: SharedInfo,
    pub unused: UnusedInfo,
    pub driver: DriverInfo,
}

impl Display for LdeInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "
mem_start:\t{:#x}
mem_size:\t{:#x}
elf_start:\t{:#x}
rt_start:\t{:#x}
drivers_start:\t{:#x}
unused_start:\t{:#x}
",
            self.mem.start as usize,
            self.mem.page_num * 0x1000 as usize,
            self.bin.ptr as usize,
            self.rt.ptr as usize,
            self.driver.ptr as usize,
            self.unused.start as usize
        ))?;

        // for (idx, m) in self.mods.iter().enumerate() {
        //     f.write_fmt(format_args!("module {idx} start:\t{:#x}\n", m.ptr as usize))?;
        //     if m.args.is_some() {
        //         f.write_fmt(format_args!(
        //             "module {idx} args:\t{}\n",
        //             m.args.as_ref().unwrap().as_str()
        //         ))?;
        //     }
        // }

        Ok(())
    }
}
