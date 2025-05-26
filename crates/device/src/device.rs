use core::{fmt::Display, ptr::slice_from_raw_parts_mut, slice};

use fdt::{Fdt, node::FdtNode, update::FdtUpdater};

use crate::{
    console::{find_uart, Uart}, cpu::Cpu, dma::find_dma, pmu::Pmu
};

#[derive(Clone)]
pub struct MemRegion {
    pub start: *const u8,
    pub size: usize,
}

impl MemRegion {
    pub fn end(&self) -> *const u8 {
        unsafe { self.start.add(self.size) }
    }
}

impl Display for MemRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{:#x}..{:#x}",
            self.start as usize,
            (self.start as usize + self.size)
        ))
    }
}

/// Device information parsered from fdt.
///
/// TODO: This struct should also be used to modify the fdt data.
pub struct DeviceInfo<'a> {
    pub hart_num: usize,
    pub fdt: Fdt<'a>,
}

impl<'a> DeviceInfo<'a> {
    /// Generate DeviceInfo from fdt by given the @ptr
    pub fn new(ptr: *const u8) -> Option<Self> {
        let total_size = unsafe { slice::from_raw_parts(ptr.offset(4), 4) };
        let total_size = u32::from_be_bytes(total_size.try_into().unwrap());

        let data = unsafe { slice::from_raw_parts(ptr, total_size as usize) };
        Some(Self {
            //data,
            hart_num: 0,
            fdt: Fdt::new(data).ok()?,
        })
    }

    /// The size of fdt data
    #[inline(always)]
    pub fn total_size(&self) -> usize {
        self.fdt.total_size()
    }

    #[inline(always)]
    pub fn get_uart(&self) -> Option<Uart> {
        find_uart(&self.fdt).ok()
    }

    #[inline(always)]
    pub fn get_dma(&self) -> Option<*mut u8> {
        find_dma(&self.fdt).ok()
    }

    /// Get memory region from fdt
    pub fn get_mem_regions(&self) -> impl IntoIterator<Item = MemRegion> {
        self.fdt.memory().regions().map(|region| MemRegion {
            start: region.starting_address,
            size: region.size.unwrap_or(0),
        })
    }

    //
    pub fn get_mem_region_reserved(&self) -> impl IntoIterator<Item = MemRegion> {
        let node = self.fdt.find_node("/reserved-memory").unwrap();

        node.children()
            .filter_map(|node| node.reg())
            .map(|mems| {
                mems.map(|mem| MemRegion {
                    start: mem.starting_address,
                    size: mem.size.unwrap_or(0),
                })
            })
            .flatten()
    }

    pub fn get_clint_region(&self) -> Option<MemRegion> {
        self.fdt
            .find_node("/soc/clint")
            .or_else(|| {
                self.fdt.find_node("/soc/clint-mswi")
            })?
            .reg()?
            .map(|reg| MemRegion {
                start: reg.starting_address,
                size: reg.size.unwrap_or(0),
            })
            .next()
    }

    #[inline(always)]
    pub fn get_cpu(&self) -> Cpu {
        Cpu {
            time_freq: self.fdt.cpus().next().unwrap().timebase_frequency()
        }
    }

    /// Change memory region
    pub fn update_mem_region_size(&mut self, start: usize, new_size: usize) -> Option<usize> {
        let node = self.fdt.find_node("/memory")?;
        let mut old_size = None;
        let sizes = node.parent_cell_sizes();
        if sizes.address_cells > 2 || sizes.size_cells > 2 {
            return None;
        }
        let regs = node.properties().filter(|prop| prop.name == "reg");

        for reg in regs {
            let mut stream = FdtUpdater::new(unsafe {
                &mut *slice_from_raw_parts_mut(reg.value.as_ptr() as *mut u8, reg.value.len())
            });
            let starting_address = match sizes.address_cells {
                1 => stream.u32()?.get() as usize,
                2 => stream.u64()?.get() as usize,
                _ => return None,
            };
            if starting_address != start {
                continue;
            }

            old_size = match sizes.size_cells {
                0 => None,
                1 => stream.update_u32(new_size as u32).map(|v| v.get() as usize),
                2 => stream.update_u64(new_size as u64).map(|v| v.get() as usize),
                _ => return None,
            };

            break;
        }

        old_size
    }

    #[inline(always)]
    pub fn iter_all_nodes(&'a self) -> impl Iterator<Item = FdtNode<'a, '_>> {
        self.fdt.all_nodes()
    }

    pub fn update_reserved_mem_region_size(
        &mut self,
        start: usize,
        new_size: usize,
    ) -> Option<usize> {
        let node = self.fdt.find_node("/reserved-memory").unwrap();
        let mut old_size = None;
        let sizes = node.parent_cell_sizes();
        if sizes.address_cells > 2 || sizes.size_cells > 2 {
            return None;
        }

        let regs = node
            .children()
            .map(|node| node.properties())
            .flatten()
            .filter(|prop| prop.name == "reg");

        for reg in regs {
            let mut stream = FdtUpdater::new(unsafe {
                &mut *slice_from_raw_parts_mut(reg.value.as_ptr() as *mut u8, reg.value.len())
            });
            let starting_address = match sizes.address_cells {
                1 => stream.u32()?.get() as usize,
                2 => stream.u64()?.get() as usize,
                _ => return None,
            };
            if starting_address != start {
                continue;
            }

            old_size = match sizes.size_cells {
                0 => None,
                1 => stream.update_u32(new_size as u32).map(|v| v.get() as usize),
                2 => stream.update_u64(new_size as u64).map(|v| v.get() as usize),
                _ => return None,
            };

            break;
        }
        //.map(|prop| {
        //
        //});
        ////.filter_map(|node| node.properties())
        //.map(|mems| {
        //    mems.map(|mem| MemRegion {
        //        start: mem.starting_address,
        //        size: mem.size.unwrap_or(0),
        //    })
        //})
        //.flatten()

        old_size
    }

    //
    //pub fn update_reserved_mem(
    //    &mut self,
    //    old_range: Range<usize>,
    //    new_range: Range<usize>,
    //) -> Result<()> {
    //    for node in fdt::Fdt::new(self.data)?
    //        .find_node("/reserved-memory")
    //        .ok_or(Error::fdt_node_not_found())?
    //        .children()
    //    {
    //        let value = node
    //            .property("reg")
    //            .ok_or(Error::fdt_prop_not_found())?
    //            .value;
    //        if parser_prop_reg(value).ok_or(Error::fdt_value_parser_err())? == old_range {
    //            unsafe {
    //                update_prop_reg(
    //                    (value as *const [u8] as *mut [u8]).as_mut().unwrap(),
    //                    new_range.start,
    //                    new_range.len(),
    //                )
    //            }
    //        }
    //    }
    //
    //    Ok(())
    //}
    //
    //fn find_node_start_with(&self, name: &str) -> Option<FdtNode<'_, '_>> {
    //    self.fdt
    //        .all_nodes()
    //        .find(|node| node.name.starts_with(name))
    //}
}

#[derive(Debug, Clone)]
pub struct Device {
    pub hart_num: usize,
    pub uart: Uart,
    pub cpu: Cpu,
}

impl Device {
    pub fn from_device_info(device_info: &DeviceInfo) -> Option<Self> {
        let uart = device_info.get_uart()?;
        let cpu = device_info.get_cpu();
        Some(Self {
            hart_num: device_info.hart_num,
            uart,
            cpu
        })
    }
}
