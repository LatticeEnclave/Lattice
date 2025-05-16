use crate::{consts::LUE_ELF_LOAD_OFFSET, frame::RtFrameAlloc, pt::RtPtWriter, task::Task};
use elf_loader::{ElfLoader, ElfObject, LoadableHeaders};
use vm::{
    page_table::PTEFlags,
    vm::{Sv39VmMgr, VirtAddr, VirtPageNum},
};

pub struct RuntimeElfLoader<'a> {
    pub vmm: &'a mut Sv39VmMgr<RtPtWriter, RtFrameAlloc>,
}

impl<'a> RuntimeElfLoader<'a> {
    pub fn new(vmm: &'a mut Sv39VmMgr<RtPtWriter, RtFrameAlloc>) -> Self {
        Self { vmm }
    }

    pub fn load_elf(&mut self, task: &mut Task, elf_object: ElfObject) -> Option<usize> {
        let _ = elf_object.load(self);
        let entry = elf_object.elf_entry();
        // create maps of task virtual space
        for ph in elf_object.iter_loadable_headers() {
            // get the start and end vpn in task virtual space
            let task_vpn_start = VirtPageNum::from_vaddr(ph.virtual_addr() as usize);
            let task_vpn_end =
                VirtPageNum::from_vaddr(ph.virtual_addr() as usize + ph.mem_size() as usize - 1);

            let mut t_vmm_lock = task.vmm.lock();

            for cur_vpn in task_vpn_start..=task_vpn_end {
                // get the current physical page
                let cur_rt_vpn = VirtPageNum::from_vaddr(LUE_ELF_LOAD_OFFSET + (cur_vpn.0 << 12));
                if cur_vpn == task_vpn_start {
                    match self.vmm.get_pte(cur_rt_vpn) {
                        None => {
                            continue;
                        }
                        Some(_) => {}
                    }
                }
                // unmap the page in runtime virtual space
                let ppn = self.vmm.unmap_frame(cur_rt_vpn)?.get_ppn();
                // map to task virtual space
                t_vmm_lock.map_frame(cur_vpn, ppn, PTEFlags::urwx().accessed().dirty());
            }
        }

        Some(entry)
    }
}

/// Implement elf load related functions for Kernel, every time generate a new task.
/// Map the program should be loaded in runtime virtual space to load and relocate
impl<'a> ElfLoader for RuntimeElfLoader<'a> {
    //map the programs in runtime space
    fn map_program(
        &mut self,
        load_headers: LoadableHeaders,
    ) -> core::result::Result<(), elf_loader::ElfLoaderErr> {
        for ph in load_headers {
            let vpn_start =
                VirtPageNum::from_vaddr(LUE_ELF_LOAD_OFFSET + ph.virtual_addr() as usize);
            let vpn_end = VirtPageNum::from_vaddr(
                LUE_ELF_LOAD_OFFSET + ph.virtual_addr() as usize + ph.mem_size() as usize - 1,
            );
            for vpn in vpn_start..=vpn_end {
                if vpn == vpn_start {
                    // check the first page may be mapped
                    match self.vmm.get_pte(vpn) {
                        Some(_) => {
                            continue;
                        }
                        None => {}
                    }
                }
                self.vmm.alloc_new_page(vpn, PTEFlags::rwx().accessed().dirty()).unwrap();
            }
        }
        Ok(())
    }

    // write the programs to physical pages
    fn load(
        &mut self,
        flags: elf_loader::Flags,
        entry: usize,
        region: &[u8],
    ) -> core::result::Result<(), elf_loader::ElfLoaderErr> {
        let vaddr_start = LUE_ELF_LOAD_OFFSET + entry;
        unsafe {
            let dst = core::slice::from_raw_parts_mut(vaddr_start as *mut u8, region.len());
            dst.copy_from_slice(region);
        }
        Ok(())
    }

    // relocate
    fn relocate(
        &mut self,
        entry: elf_loader::RelocationEntry,
    ) -> core::result::Result<(), &'static str> {
        // not implemented
        Ok(())

        // let rel_addr = ELF_LOAD_OFFSET + entry.offset;
        // let rel_value = ELF_LOAD_OFFSET + entry.symval.unwrap_or(0) + entry.addend;
        // unsafe {
        //     let dst = rel_addr as *mut usize;
        //     *dst = rel_value;
        // }
        // Ok(())
    }
}
