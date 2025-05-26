use crate::Error;
use core::ops::Range;
use core::slice;
use fdt::{Fdt, node::FdtNode};
use console::log;

/// Device information parsered from fdt.
///
/// TODO: This struct should also be used to modify the fdt data.
pub struct DeviceInfo<'a> {
    // uart: Range<usize>,
    data: &'a [u8],
    fdt: Fdt<'a>,
}

impl<'a> DeviceInfo<'a> {
    /// Generate DeviceInfo from fdt by given the @ptr
    pub fn new(ptr: *const u8) -> Self {
        let total_size = unsafe { slice::from_raw_parts(ptr.offset(4), 4) };
        let total_size = u32::from_be_bytes(total_size.try_into().unwrap());

        let data = unsafe { slice::from_raw_parts(ptr, total_size as usize) };
        Self {
            data,
            fdt: Fdt::new(data).unwrap(),
        }
    }

    pub fn print_info(&self) {
        self.fdt.all_nodes().for_each(|node| {
            log::info!("fdt node: {}", node.name);
            if let Some(range) = node
                .property("reg")
                .and_then(|prop| parser_prop_reg(prop.value))
            {
                log::info!("{:#x}..{:#x}", range.start, range.start + range.len());
            };
        });
    }

    /// The size of fdt data
    pub fn total_size(&self) -> usize {
        self.fdt.total_size()
    }

    pub fn get_uart(&self) -> Option<Range<usize>> {
        self.find_node_start_with("uart")
            .or(self.find_node_start_with("serial"))
            .and_then(|node| node.property("reg"))
            .and_then(|prop| parser_prop_reg(prop.value))
    }

    /// Get memory region from fdt
    pub fn get_mem_region(&self) -> Option<Range<usize>> {
        let value = self.find_node_start_with("memory")?.property("reg")?.value;
        parser_prop_reg(value)
    }

    pub fn get_mem_reserved(&'a self) {
        let fdt = fdt::Fdt::new(self.data).unwrap();
        let node = fdt.find_node("/reserved-memory").unwrap();
        node.children()
            .filter_map(|node| {
                node.property("reg")
                    .and_then(|prop| parser_prop_reg(prop.value))
            })
            .for_each(|range| {
                log::info!("range: {:#x?}", range);
            });
    }

    pub fn get_clint_region(&self) -> Option<Range<usize>> {
        let value = self.find_node_start_with("clint")?.property("reg")?.value;
        parser_prop_reg(value)
    }

    /// Change memory region
    pub fn update_mem_region(&mut self, start: usize, size: usize) {
        let ptr = self
            .find_node_start_with("memory")
            .and_then(|node| node.property("reg"))
            .unwrap()
            .value as *const [u8] as *mut [u8];
        // let ptr = self.mem.unwrap().value as *const [u8] as *mut [u8];

        unsafe { update_prop_reg(ptr.as_mut().unwrap(), start, size) }
    }

    pub fn update_reserved_mem(
        &mut self,
        old_range: Range<usize>,
        new_range: Range<usize>,
    ) -> Result<(), Error> {
        for node in fdt::Fdt::new(self.data)?
            .find_node("/reserved-memory")
            .ok_or(Error::fdt_node_not_found())?
            .children()
        {
            let value = node
                .property("reg")
                .ok_or(Error::fdt_prop_not_found())?
                .value;
            if parser_prop_reg(value).ok_or(Error::fdt_value_parser_err())? == old_range {
                unsafe {
                    update_prop_reg(
                        (value as *const [u8] as *mut [u8]).as_mut().unwrap(),
                        new_range.start,
                        new_range.len(),
                    )
                }
            }
        }

        Ok(())
    }

    fn find_node_start_with(&self, name: &str) -> Option<FdtNode<'_, '_>> {
        self.fdt
            .all_nodes()
            .find(|node| node.name.starts_with(name))
    }
}

// fn parser_fdt_impl(data: &[u8]) -> DeviceInfo {
//     let fdt = fdt::Fdt::new(data).unwrap();
//     let mut device = DeviceInfo { data, fdt };

//     let fdt = fdt::Fdt::new(data).unwrap();

//     fdt.all_nodes().for_each(|node| {
//         if node.name.starts_with("uart") || node.name.starts_with("serial") {
//             device.uart = get_node_reg(node).unwrap();
//         }
//     });

//     device
// }

fn parser_prop_reg(value: &[u8]) -> Option<Range<usize>> {
    let base = usize::from_be_bytes(value.get(..8)?.try_into().ok()?);
    let size = usize::from_be_bytes(value.get(8..16)?.try_into().ok()?);
    Some(base..base + size)
}

unsafe fn update_prop_reg(value: &mut [u8], base: usize, size: usize) {
    // value.get(index)
    let bytes = base.to_be_bytes();

    for i in 0..8 {
        value[i] = bytes[i];
    }

    let bytes = size.to_be_bytes();
    for i in 0..8 {
        value[i + 8] = bytes[i];
    }
}