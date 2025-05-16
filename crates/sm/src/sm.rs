use core::{ops::Range, ptr::NonNull, sync::atomic::AtomicUsize};
use data_structure::linked_list::LinkedList;
use enclave::{
    EnclaveId, EnclaveIdGenerator, EnclaveType, LinuxServiceEnclaveList, LinuxUserEnclaveList,
};
use extension::Extension;
use heapless::Vec;
use hsm::{Hsm, MAX_HART_NUM};
use htee_device::device::{Device, DeviceInfo};
use mempool::Mempool;
// use htee_macro::usize_env_or;
use riscv::register::{Permission, satp};
use spin::{Mutex, RwLock};
use vm::{aligned, page_table::BarePtReader, vm::VirtAddr, Translate};

use crate::{
    Error,
    PMP_COUNT, // ecall,
    // hart,
    // inst_ext,
    // pma::PhysMemAreaMgr,
    // pmp::update_pmp_by_pmas,
    heap::Heap,
};
use clint::ClintClient;
use pma::{Owner, PhysMemArea, PhysMemAreaMgr, PmaProp};

// pub const SM_START: usize =
//     usize_env_or!("FW_TEXT_START", 0x80000000) + usize_env_or!("SBI_SIZE", 0x60000);

// type SmSv39VmMgr = Sv39VmMgr<BarePtWriter, OneShotAllocatorWrapper>;

const CHANNEL_NUM: usize = 16;

pub struct SecMonitor {
    pub pma_mgr: RwLock<PhysMemAreaMgr>,
    pub eid_gen: EnclaveIdGenerator,
    pub clint: ClintClient,
    pub hsm: Hsm,
    // pub hart_num: usize,
    // pub mmio: Mmio,
    pub dma: usize,
    pub lues: Mutex<LinuxUserEnclaveList>,
    pub ldes: Mutex<LinkedList<EnclaveType>>,
    pub lses: Mutex<LinuxServiceEnclaveList>,

    pub nw_cache: Mutex<pmp::NwCache>,

    pub nw_fault_num: AtomicUsize,

    pub heap: Mutex<Heap>,
    // pub channels: Mutex<[Channel<usize>; CHANNEL_NUM]>,
    pub device: Device,

    // pub pmp_bufs:
}

impl Extension<PhysMemAreaMgr> for SecMonitor {
    fn view<O>(&self, f: impl FnOnce(&PhysMemAreaMgr) -> O) -> O {
        f(&self.pma_mgr.read())
    }

    fn update<O>(&self, f: impl FnOnce(&mut PhysMemAreaMgr) -> O) -> O {
        f(&mut self.pma_mgr.write())
    }
}

impl Extension<ClintClient> for SecMonitor {
    #[inline]
    fn view<O>(&self, f: impl FnOnce(&ClintClient) -> O) -> O {
        f(&self.clint)
    }

    fn update<O>(&self, _: impl FnOnce(&mut ClintClient) -> O) -> O {
        unimplemented!()
    }
}

impl Extension<Device> for SecMonitor {
    #[inline]
    fn view<O>(&self, f: impl FnOnce(&Device) -> O) -> O {
        f(&self.device)
    }

    fn update<O>(&self, _: impl FnOnce(&mut Device) -> O) -> O {
        unimplemented!()
    }
}

impl Extension<LinuxServiceEnclaveList> for SecMonitor {
    fn view<O>(&self, f: impl FnOnce(&LinuxServiceEnclaveList) -> O) -> O {
        f(&self.lses.lock())
    }

    fn update<O>(&self, f: impl FnOnce(&mut LinuxServiceEnclaveList) -> O) -> O {
        f(&mut self.lses.lock())
    }
}

impl Extension<Hsm> for SecMonitor {
    #[inline]
    fn view<O>(&self, f: impl FnOnce(&Hsm) -> O) -> O {
        f(&self.hsm)
    }

    fn update<O>(&self, _: impl FnOnce(&mut Hsm) -> O) -> O {
        // f(&mut self.hsm)
        unimplemented!()
    }
}

impl Extension<LinuxUserEnclaveList> for SecMonitor {
    #[inline]
    fn view<O>(&self, f: impl FnOnce(&LinuxUserEnclaveList) -> O) -> O {
        f(&self.lues.lock())
    }

    fn update<O>(&self, f: impl FnOnce(&mut LinuxUserEnclaveList) -> O) -> O {
        f(&mut self.lues.lock())
    }
}

impl Extension<EnclaveIdGenerator> for SecMonitor {
    fn view<O>(&self, f: impl FnOnce(&EnclaveIdGenerator) -> O) -> O {
        f(&self.eid_gen)
    }

    fn update<O>(&self, _: impl FnOnce(&mut EnclaveIdGenerator) -> O) -> O {
        unimplemented!()
    }
}

impl Extension<pmp::NwCache> for SecMonitor {
    fn view<O>(&self, f: impl FnOnce(&pmp::NwCache) -> O) -> O {
        f(&self.nw_cache.lock())
    }

    fn update<O>(&self, f: impl FnOnce(&mut pmp::NwCache) -> O) -> O {
        f(&mut self.nw_cache.lock())
    }
}

impl SecMonitor {
    // pub fn new(device: &DeviceInfo) -> Self {
    //     Self {
    //         pma_mgr: RwLock::new(PhysMemAreaMgr::uninit()),
    //         eid_gen: EnclaveIdGenerator::new(),
    //         clint: ClintClient::new(device.hart_num),
    //         // mmio: Mmio::default(),
    //         hsm: Hsm::new(device.hart_num),
    //         dma: 0,
    //         // hart_num: device.hart_num,
    //         lues: Mutex::new(LinuxUserEnclaveList::new()),
    //         ldes: Mutex::new(LinkedList::new()),
    //         lses: Mutex::new(LinuxServiceEnclaveList::new()),
    //         // channels: Mutex::new([Channel::EMPTY; CHANNEL_NUM]),
    //         device: Device::from_device_info(device).unwrap(),
    //     }
    // }

    #[inline]
    pub fn init(&mut self, device: &DeviceInfo) {
        self.eid_gen = EnclaveIdGenerator::new();
        self.clint = ClintClient::new(device.hart_num);
        self.hsm = Hsm::new(device.hart_num);
        self.dma = 0;
        self.lues = Mutex::new(LinuxUserEnclaveList::new());
        self.ldes = Mutex::new(LinkedList::new());
        self.lses = Mutex::new(LinuxServiceEnclaveList::new());
        self.device = Device::from_device_info(device).unwrap();
    }

    #[inline(always)]
    pub fn init_pma(&mut self, f: impl FnOnce() -> PhysMemAreaMgr) {
        let mgr = f();
        self.pma_mgr = RwLock::new(mgr);
    }

    #[inline]
    pub fn init_heap(&mut self, start: *mut u8, size: usize) {
        use pmp::PmpBuf;

        type BufPool = Vec<PmpBuf, MAX_HART_NUM>;

        assert!(aligned!(start as usize, 0x8));
        assert!(core::mem::size_of::<BufPool>() <= size);
        let mut ptr = NonNull::new(start as *mut BufPool).unwrap();
        unsafe {
            *ptr.as_mut() = Vec::new();
            for (i, hs) in self.hsm.iter_hs_mut().enumerate() {
                ptr.as_mut().push(Vec::new()).unwrap();
                hs.pmp_buf = NonNull::new(ptr.as_mut().get_mut(i).unwrap()).unwrap()
            }
        }
    }

    #[inline]
    pub fn alloc_mempool_spin(&self) -> Mempool {
        loop {
            if let Some(pool) = self.heap.lock().alloc_mempool() {
                return pool;
            }
        }
    }

    #[inline]
    pub fn free_mempool(&self, mempool: Mempool) {
        unsafe { self.heap.lock().free_mempool(mempool) };
    }

    // pub fn init_mmio(&mut self, device: &DeviceInfo) {
    //     self.mmio.uart = device.get_uart();
    //     self.mmio.dma = device.get_dma().map(|dma| dma as usize).unwrap_or(0);
    // }

    // pub fn alloc_channel(&self, eid: EnclaveId, arg0: usize, arg1: usize) -> Result<usize, Error> {
    //     let mut channels = self.channels.lock();
    //     for (i, channel) in channels.iter_mut().enumerate() {
    //         if channel.status == ChannelStatus::Free {
    //             channel.status = ChannelStatus::Using;
    //             channel.lue = Some(eid.0);
    //             channel.arg0 = arg0 as u64;
    //             channel.arg1 = arg1 as u64;
    //             return Ok(i);
    //         }
    //     }
    //     Err(Error::ChannelFull)
    // }

    // pub fn dealloc_channel(&self, id: usize) -> Result<(), Error> {
    //     let mut channels = self.channels.lock();
    //     channels[id].status = ChannelStatus::Free;
    //     channels[id].lue = None;
    //     channels[id].lde = None;
    //     Ok(())
    // }

    // pub fn connect_channel(&self, cid: usize, eid: EnclaveId) -> Result<u64, Error> {
    //     let mut channels = self.channels.lock();
    //     let lue = self.search_lue(channels[cid].lue.unwrap()).unwrap();
    //     if !lue.ldes.contains(&eid) {
    //         return Err(Error::Other("The connection is not allowed"));
    //     }
    //     if channels[cid].lde.is_some() {
    //         return Err(Error::Other("Channel already connected"));
    //     }
    //     channels[cid].lde = Some(eid.0);
    //     Ok(channels[cid].arg0)
    // }

    // pub fn attach_channel<T>(
    //     &self,
    //     eid: EnclaveId,
    //     f: impl FnOnce(&mut Channel<usize>) -> Result<T, Error>,
    // ) -> Result<T, Error> {
    //     let mut channels = self.channels.lock();
    //     let channel = channels.iter_mut().find(|c| c.lde == Some(eid.0)).unwrap();
    //     f(channel)
    // }

    pub fn copy_data(
        &self,
        src: (VirtAddr, satp::Satp, EnclaveId),
        dst: (VirtAddr, satp::Satp, EnclaveId),
        size: usize,
    ) -> Result<(), Error> {
        use core::slice;

        let mut offset = 0;
        while offset < size {
            let src_paddr = src
                .0
                .add(offset)
                .translate(src.1.ppn(), src.1.mode(), &BarePtReader)
                .unwrap();
            let dst_paddr = dst
                .0
                .add(offset)
                .translate(dst.1.ppn(), dst.1.mode(), &BarePtReader)
                .unwrap();

            let src_pma = self.pma_mgr.read().get_pma(src_paddr.0).unwrap();
            let dst_pma = self.pma_mgr.read().get_pma(dst_paddr.0).unwrap();
            if src_pma.get_prop().get_owner() != src.2 || dst_pma.get_prop().get_owner() != dst.2 {
                return Err(Error::Other("PMA access violation"));
            }

            let (src, dst) = if (size - offset) >= 0x1000 {
                if src_pma.get_region().len() < 0x1000 || dst_pma.get_region().len() < 0x1000 {
                    return Err(Error::Other("The source enclave is not allowed to access"));
                }
                offset += 0x1000;
                (
                    unsafe { slice::from_raw_parts(src_paddr.0 as *mut u8, 0x1000) },
                    unsafe { slice::from_raw_parts_mut(dst_paddr.0 as *mut u8, 0x1000) },
                )
            } else {
                if src_pma.get_region().len() < size - offset
                    || dst_pma.get_region().len() < size - offset
                {
                    return Err(Error::Other("The source enclave is not allowed to access"));
                }
                offset = size;
                (
                    unsafe { slice::from_raw_parts(src_paddr.0 as *mut u8, size - offset) },
                    unsafe { slice::from_raw_parts_mut(dst_paddr.0 as *mut u8, size) },
                )
            };
            dst.copy_from_slice(src);
        }
        Ok(())
    }

    #[inline]
    pub fn iter_current_pma(&self) -> impl Iterator<Item = PhysMemArea> {
        use pmp::iter_hps;

        iter_hps()
            .filter(|p| !p.is_off())
            .map(|p| p.get_region())
            .map(|r| self.pma_mgr.read().get_pma(r.start).unwrap())
    }

    pub fn get_current_pmas(&self) -> Vec<PhysMemArea, PMP_COUNT> {
        use pmp::hps_from_regs;

        let hps = hps_from_regs();
        let pmas = hps
            .into_iter()
            .map(|p| p.get_region())
            .map(|r| self.pma_mgr.read().get_pma(r.start).unwrap())
            .collect();
        pmas
    }

    // pub fn create_lse(&self, arg0: usize) -> Result<(), Error> {
    //     use htee_channel::info::LseInfo;

    //     let host_satp = satp::read();

    //     let load_info = unsafe {
    //         let paddr = VirtAddr(arg0)
    //             .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //             .unwrap();

    //         &*(paddr.0 as *const LseInfo)
    //     };

    //     log::debug!("{}", load_info);
    //     log::debug!("host satp: {:#x}", host_satp.bits());

    //     update_enc_pma_by_vaddr(
    //         EnclaveId::EVERYONE,
    //         Permission::RX,
    //         load_info.mem.start.into(),
    //         load_info.mem.page_num * 0x1000,
    //         &mut self.pma_mgr.write(),
    //         host_satp,
    //     );

    //     // notify other harts to clear their pmp registers
    //     for i in 0..self.hart_num {
    //         if i == mhartid::read() {
    //             continue;
    //         }
    //         hart::send_ops(i, hart::HartStateOps {
    //             clean_pmp: true,
    //             ..hart::recv_op(i)
    //         });
    //     }

    //     // alloc meta page and update meta page ownership
    //     let meta_page =
    //         VirtAddr(load_info.mem.start as usize + load_info.mem.page_num * 0x1000 - 0x1000)
    //             .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //             .unwrap();
    //     log::debug!("meta page: {:#x}", meta_page.0);
    //     self.pma_mgr
    //         .write()
    //         .insert_page(meta_page, EnclaveId::EVERYONE, Permission::NONE);

    //     let enclave =

    //     Ok(())
    // }

    // pub fn create_lde(&self, arg0: usize) -> Result<EnclaveId, Error> {
    //     use enclave::lde_builder;
    //     use htee_channel::info::LdeInfo;

    //     let host_satp = satp::read();

    //     // get the load info from cli tool
    //     let load_info = unsafe {
    //         let paddr = VirtAddr(arg0)
    //             .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //             .unwrap();

    //         &*(paddr.0 as *const LdeInfo)
    //     };

    //     log::debug!("{}", load_info);
    //     log::debug!("host satp: {:#x}", host_satp.bits());

    //     let eid = self.eid_gen.fetch();
    //     let total_mem_size = load_info.mem.page_num * 0x1000;
    //     let enc_rt_start = RT_VADDR_START;
    //     let rt_size = align_up!(load_info.rt.size, 0x1000);
    //     let driver_start = load_info.driver.ptr as usize;
    //     let driver_size = load_info.driver.size;
    //     let enc_bin_start = BIN_VADDR_START;
    //     let bin_size = align_up!(load_info.bin.size, 0x1000);
    //     let stack_start = RT_VADDR_START - 0x1000;
    //     let boot_arg_addr = BOOTARG_VADDR;
    //     let enc_mods_start = enc_bin_start + bin_size;

    //     log::info!("Creating linux driver enclave. Eid: {}", eid);

    //     update_enc_pma_by_vaddr(
    //         eid,
    //         Permission::RWX,
    //         load_info.mem.start.into(),
    //         total_mem_size,
    //         &mut self.pma_mgr.write(),
    //         host_satp,
    //     );

    //     // notify other harts to clear their pmp registers
    //     for i in 0..self.hart_num {
    //         if i == mhartid::read() {
    //             continue;
    //         }
    //         hart::send_ops(i, hart::HartStateOps {
    //             clean_pmp: true,
    //             ..hart::recv_op(i)
    //         });
    //     }

    //     self.clint.send_ipi_other_harts();
    //     log::debug!("sync pmp registers");

    //     // alloc meta page and update meta page ownership
    //     let meta_page = VirtAddr(load_info.unused.start as usize + load_info.unused.size - 0x1000)
    //         .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //         .unwrap();
    //     log::debug!("meta page: {:#x}", meta_page.0);
    //     self.pma_mgr.write().insert_page(
    //         meta_page,
    //         PmaProp::empty().owner(eid).permission(Permission::NONE),
    //     );

    //     // create the enclave virtual memory allocator
    //     let allocator = OneShotAllocatorWrapper(RefCell::new(OneShotAllocator::new(
    //         host_satp.ppn(),
    //         host_satp.mode(),
    //         load_info.unused.start as usize,
    //         load_info.unused.size - 0x1000,
    //     )));
    //     // we firstly allocate the stack and args page
    //     let stack_ppn = allocator.alloc().unwrap();
    //     let args_ppn = allocator.alloc().unwrap();
    //     let serial_reg = self.device.uart.get_reg();

    //     let (lde, vmm) = lde_builder(meta_page, driver_start, driver_size)
    //         .eid(eid)
    //         .host_info(HostInfo {
    //             vaddr: load_info.mem.start.into(),
    //             size: load_info.mem.page_num * 0x1000,
    //             asid: host_satp.asid(),
    //             pt_root: host_satp.ppn().into(),
    //             pt_mode: host_satp.mode(),
    //             ..Default::default()
    //         })
    //         .prepare_vmm(allocator)
    //         .create_trampoline(VirtAddr::from(load_info.rt.ptr))
    //         // map runtime
    //         .map_host_pages(
    //             VirtPageNum::from_vaddr(load_info.rt.ptr),
    //             VirtPageNum::from_vaddr(enc_rt_start),
    //             rt_size / 0x1000,
    //             PTEFlags::rwx(),
    //         )
    //         // map stack
    //         .map_frame(
    //             stack_ppn,
    //             VirtPageNum::from_vaddr(stack_start),
    //             PTEFlags::rwx(),
    //         )
    //         // map args
    //         .map_frame(
    //             args_ppn,
    //             VirtPageNum::from_vaddr(boot_arg_addr),
    //             PTEFlags::rwx(),
    //         )
    //         // map binary
    //         .map_host_pages(
    //             VirtPageNum::from_vaddr(load_info.bin.ptr),
    //             VirtPageNum::from_vaddr(enc_bin_start),
    //             load_info.bin.size.div_ceil(0x1000),
    //             PTEFlags::rwx(),
    //         )
    //         .map_mmio(&serial_reg)
    //         .finish();

    //     // we link the remain free frames
    //     let (head, free_size) = link_remain_frame(vmm, host_satp);

    //     // then we construct the enclave boot arguments
    //     use htee_channel::enclave::runtime::*;

    //     let args = LdeBootArgs {
    //         mem: MemArg {
    //             total_size: total_mem_size,
    //         },
    //         mods: ModArg {
    //             start_vaddr: enc_mods_start,
    //             num: 0,
    //         },
    //         tp: TpArg { addr: lde.tp },
    //         bin: BinArg {
    //             start: enc_bin_start,
    //             size: bin_size,
    //         },
    //         driver_start,
    //         driver_size,
    //         sections: load_info.driver.sections.clone(),
    //         unmapped: UnmappedArg {
    //             head: head,
    //             size: free_size,
    //         },
    //         device: self.device.clone(),
    //     };
    //     log::debug!("boot args: {args}");

    //     unsafe {
    //         *(PhysAddr::from_ppn(args_ppn).0 as *mut LdeBootArgs) = args;
    //         lde.enclave_ctx.tregs.a1 = boot_arg_addr;
    //     }

    //     // add enclave to the list
    //     self.ldes.lock().push_node(lde.node.as_node_mut());

    //     // clean current hart pmp
    //     hart::current().clean_pmp();
    //     log::debug!("clean current hart pmp");

    //     Ok(lde.id)
    // }

    /// Create new enclave
    // pub fn create_user_enclave(&self, arg0: usize) -> Result<EnclaveId, Error> {
    //     use htee_channel::h2e::*;

    //     let host_satp = satp::read();

    //     // get the load info from cli tool
    //     let load_info = unsafe {
    //         let paddr = VirtAddr(arg0)
    //             .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //             .unwrap();

    //         &*(paddr.0 as *const LueInfo)
    //     };

    //     log::debug!("{}", load_info);
    //     log::debug!("host satp: {:#x}", host_satp.bits());

    //     let eid = self.eid_gen.fetch();
    //     let total_mem_size = load_info.mem.page_num * 0x1000;
    //     let enc_rt_start = RT_VADDR_START;
    //     let rt_size = align_up!(load_info.rt.size, 0x1000);
    //     let enc_bin_start = BIN_VADDR_START;
    //     let bin_size = align_up!(load_info.bin.size, 0x1000);
    //     let enc_share_start = BIN_VADDR_START + bin_size;
    //     let share_size = align_up!(load_info.shared.size, 0x1000);
    //     let stack_start = RT_VADDR_START - 0x1000;
    //     let boot_arg_addr = BOOTARG_VADDR;
    //     let enc_mods_start = enc_bin_start + bin_size;
    //     // let mods_num = load_info.mods.len();

    //     log::info!("Creating linux user enclave. Eid: {}", eid);

    //     if self
    //         .pma_mgr
    //         .read()
    //         .iter_pma()
    //         .any(|pma| pma.get_prop().get_owner() == eid)
    //     {
    //         log::error!("Existing pma for enclave {}", eid);
    //         panic!()
    //     }

    //     // update pmas ownership
    //     update_enc_pma_by_vaddr(
    //         eid,
    //         Permission::RWX,
    //         load_info.mem.start.into(),
    //         total_mem_size,
    //         &mut self.pma_mgr.write(),
    //         host_satp,
    //     );

    //     // notify other harts to clear their pmp registers
    //     for i in 0..self.hart_num {
    //         if i == mhartid::read() {
    //             continue;
    //         }
    //         hart::send_ops(i, hart::HartStateOps {
    //             clean_pmp: true,
    //             ..hart::recv_op(i)
    //         });
    //     }
    //     self.clint.send_ipi_other_harts();
    //     log::debug!("sync pmp registers");

    //     // alloc meta page and update meta page ownership
    //     let meta_page = VirtAddr(load_info.unused.start as usize + load_info.unused.size - 0x1000)
    //         .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //         .unwrap();
    //     log::debug!("meta page: {:#x}", meta_page.0);
    //     self.pma_mgr.write().insert_page(
    //         meta_page,
    //         PmaProp::empty().owner(eid).permission(Permission::NONE),
    //     );

    //     // change shared page owner
    //     let mut remain_pages = load_info.shared.size / 0x1000;
    //     while remain_pages > 0 {
    //         let page = VirtAddr(load_info.shared.ptr as usize + (remain_pages - 1) * 0x1000)
    //             .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
    //             .unwrap();
    //         self.pma_mgr.write().insert_page(
    //             page,
    //             PmaProp::empty()
    //                 .owner(EnclaveId::EVERYONE)
    //                 .permission(Permission::RWX),
    //         );
    //         remain_pages -= 1;
    //     }

    //     // create the enclave virtual memory allocator
    //     let allocator = OneShotAllocatorWrapper(RefCell::new(OneShotAllocator::new(
    //         host_satp.ppn(),
    //         host_satp.mode(),
    //         load_info.unused.start as usize,
    //         load_info.unused.size - 0x1000,
    //     )));
    //     // we firstly allocate the stack and args page
    //     let stack_ppn = allocator.alloc().unwrap();
    //     let args_ppn = allocator.alloc().unwrap();
    //     let serial_reg = self.device.uart.get_reg();

    //     let (lue, vmm) = lue_builder(meta_page)
    //         .eid(eid)
    //         .host_info(HostInfo {
    //             vaddr: load_info.mem.start.into(),
    //             size: load_info.mem.page_num * 0x1000,
    //             asid: host_satp.asid(),
    //             shared_start: load_info.shared.ptr.into(),
    //             shared_size: load_info.shared.size,
    //             pt_root: host_satp.ppn().into(),
    //             pt_mode: host_satp.mode(),
    //         })
    //         .prepare_vmm(allocator)
    //         .create_trampoline(VirtAddr::from(load_info.rt.ptr))
    //         // map runtime
    //         .map_host_pages(
    //             VirtPageNum::from_vaddr(load_info.rt.ptr),
    //             VirtPageNum::from_vaddr(enc_rt_start),
    //             rt_size / 0x1000,
    //             PTEFlags::rwx(),
    //         )
    //         // map shared pages
    //         .map_host_pages(
    //             VirtPageNum::from_vaddr(load_info.shared.ptr),
    //             VirtPageNum::from_vaddr(enc_share_start),
    //             share_size / 0x1000,
    //             PTEFlags::rwx(),
    //         )
    //         // map stack
    //         .map_frame(
    //             stack_ppn,
    //             VirtPageNum::from_vaddr(stack_start),
    //             PTEFlags::rwx(),
    //         )
    //         // map args
    //         .map_frame(
    //             args_ppn,
    //             VirtPageNum::from_vaddr(boot_arg_addr),
    //             PTEFlags::rwx(),
    //         )
    //         // map binary
    //         .map_host_pages(
    //             VirtPageNum::from_vaddr(load_info.bin.ptr),
    //             VirtPageNum::from_vaddr(enc_bin_start),
    //             load_info.bin.size.div_ceil(0x1000),
    //             PTEFlags::rwx(),
    //         )
    //         // // map modules
    //         // .map_mods(
    //         //     VirtAddr::from(load_info.mods.as_ptr()),
    //         //     mods_num,
    //         //     VirtAddr(BIN_VADDR_START + align_up!(load_info.bin.size, 0x1000)),
    //         // )
    //         // FIXME: don't map mmio
    //         .map_mmio(&serial_reg)
    //         .finish();

    //     // we link the remain free frames
    //     let (head, free_size) = link_remain_frame(vmm, host_satp);

    //     // then we construct the enclave boot arguments
    //     use htee_channel::enclave::runtime::*;

    //     let args = LueBootArgs {
    //         mem: MemArg {
    //             total_size: total_mem_size,
    //         },
    //         mods: ModArg {
    //             start_vaddr: enc_mods_start,
    //             num: 0,
    //         },
    //         tp: TpArg { addr: lue.tp },
    //         bin: BinArg {
    //             start: enc_bin_start,
    //             size: bin_size,
    //         },
    //         shared: SharedArg {
    //             enc_vaddr: enc_share_start,
    //             host_vaddr: load_info.shared.ptr as usize,
    //             size: share_size,
    //         },
    //         unmapped: UnmappedArg {
    //             head: head,
    //             size: free_size,
    //         },
    //         device: self.device.clone(),
    //     };
    //     log::debug!("boot args: {args}");

    //     unsafe {
    //         *(PhysAddr::from_ppn(args_ppn).0 as *mut LueBootArgs) = args;
    //         lue.enclave_ctx.tregs.a1 = boot_arg_addr;
    //     }

    //     // add enclave to the list
    //     self.lues.lock().push_node(lue.node.as_node_mut());

    //     // clean current hart pmp
    //     hart::current().clean_pmp();
    //     log::debug!("clean current hart pmp");

    //     Ok(lue.id)
    // }

    ///
    // pub fn search_lue(&self, id: impl Into<EnclaveId>) -> Option<&mut LinuxUserEnclave> {
    //     use enclave::node_to_node_ptr;

    //     let id: EnclaveId = id.into();
    //     self.lues.lock().iter().find_map(|node| {
    //         let lue = unsafe { LinuxUserEnclave::from_node(node_to_node_ptr(node)) };
    //         if lue.id == id { Some(lue) } else { None }
    //     })
    // }

    // pub fn search_lde(&self, id: impl Into<EnclaveId>) -> Option<&mut LinuxDriverEnclave> {
    //     use enclave::node_to_node_ptr;

    //     let id: EnclaveId = id.into();
    //     self.ldes.lock().iter().find_map(|node| {
    //         let lde = unsafe { LinuxDriverEnclave::from_node(node_to_node_ptr(node)) };
    //         if lde.id == id { Some(lde) } else { None }
    //     })
    // }

    // pub fn launch_lde(&self, tregs: &mut TrapRegs) -> Result<!, Error> {
    //     log::debug!("Launch LDE #{}", tregs.a0);
    //     let sregs = SupervisorRegs::dump();
    //     let enclave = self
    //         .search_lde(tregs.a0)
    //         .ok_or(Error::InvalidEnclaveId(tregs.a0))?;
    //     enclave.host_ctx.sregs = sregs;
    //     enclave.host_ctx.tregs = tregs.clone();
    //     // set mepc to the next instruction
    //     enclave.host_ctx.tregs.mepc += 0x2;

    //     unsafe {
    //         hart::current().enter_enclave(
    //             enclave.idx(),
    //             RT_VADDR_START,
    //             mhartid::read(),
    //             enclave.enclave_ctx.tregs.a1,
    //             RT_VADDR_START,
    //             enclave.enclave_ctx.sregs.satp,
    //         )
    //     }
    // }

    // pub fn launch_lue(&self, tregs: &mut TrapRegs) -> Result<!, Error> {
    //     let sregs = SupervisorRegs::dump();
    //     let enclave = self
    //         .search_lue(tregs.a0)
    //         .ok_or(Error::InvalidEnclaveId(tregs.a0))?;
    //     enclave.host_ctx.sregs = sregs;
    //     enclave.host_ctx.tregs = tregs.clone();
    //     // set mepc to the next instruction
    //     enclave.host_ctx.tregs.mepc += 0x2;

    //     let arg0 = mhartid::read();
    //     let arg1 = enclave.enclave_ctx.tregs.a1;

    //     log::info!("Enclave {:#x} launching", enclave.id.0);
    //     unsafe {
    //         hart::current().enter_enclave(
    //             enclave.idx(),
    //             RT_VADDR_START,
    //             arg0,
    //             arg1,
    //             RT_VADDR_START,
    //             enclave.enclave_ctx.sregs.satp,
    //         )
    //     }
    // }

    // pub fn enter_lde(&self, tregs: &mut TrapRegs) -> Result<(), Error> {
    //     let vaddr = tregs.mepc;
    //     // find the enclave that matches the vaddr
    //     let enc = self
    //         .iter_lde()
    //         .find(|e| e.match_lde(vaddr))
    //         .ok_or(Error::InvalidLde)?;
    //     // change enclave id, and let runtime to handle it
    //     hart::current().set_enc(enc.idx());
    //     // we clean current pmp registers
    //     hart::current().clean_pmp();

    //     // we save the host context to the enclave host context
    //     enc.host_ctx.save(&tregs);

    //     // the stvec is pre-configured when the lde is created
    //     // we pass the illegal instruction exception to the sbi,
    //     // so sbi will redirect to s-mode runtime
    //     unsafe {
    //         enc.enclave_ctx.sregs.write();
    //     }

    //     Ok(())
    // }

    // pub fn exit_lde(&self, tregs: &mut TrapRegs) -> Result<(), Error> {
    //     if let Some(enc) = hart::current().get_enc_ptr() {
    //         let enc = enc.as_lde().ok_or(Error::InvalidLde)?;
    //         // for exit lde, it's no need to save the context
    //         // we can use the lde inited context all the time
    //         // // we save the host context to the enclave host context
    //         // enc.enclave_ctx.save(&tregs);

    //         // the stvec is pre-configured when the lde is created
    //         // we pass the illegal instruction exception to the sbi,
    //         // so sbi will redirect to s-mode runtime
    //         unsafe {
    //             enc.host_ctx.sregs.write();
    //         }
    //     } else {
    //         return Err(Error::Other("Not need to exit lde"));
    //     }

    //     // change to the host
    //     hart::current().set_enc(EnclaveIdx::HOST);
    //     // clean current pmp registers
    //     hart::current().clean_pmp();

    //     Ok(())
    // }

    /// Determine if the exception is raised by the context switch.
    ///
    /// ## Detail
    ///
    /// The exception raised on following possiblities:
    /// - Invalid access: vicious program access unowned memory area.
    ///   In such situation, `mepc` pointer to memory address of vicious program,
    ///   while `mtval` pointer to another memory address of protected program.
    ///   In general, `mepc.owner` != `mtval.owner`, and `mtval.owner` != `current_owner`.
    /// - Unconfigured physical memory area: `mtval` is allowed to access,
    ///   but the physical memory area that contains `mtval` is not configured in pmp registers.
    ///   In general, `mtval.owner` == `current_owner`.
    /// - Context switch: when control flow translate to host OS, the exception raised,
    ///   and the `mtval.owner` != `current_owner`.
    // fn is_context_switch(&self, hartid: usize) -> bool {
    //     todo!()
    // }

    // pub fn iter_lde(&self) -> impl Iterator<Item = &mut LinuxDriverEnclave> {
    //     self.ldes.lock().iter().map(|node| {
    //         let lde = unsafe { LinuxDriverEnclave::from_node(node_to_node_ptr(node)) };
    //         lde
    //     })
    // }

    // pub fn lock_mem(&self, user: EnclaveIdx, vaddr: usize, size: usize) -> Result<(), Error> {
    //     use enclave::EncListNode;

    //     if user == EnclaveIdx::HOST {
    //         return Err(Error::Other("ELock from host is not allowed"));
    //     }

    //     let node = EncListNode::from_idx(user);
    //     if node.is_user() {
    //         return Err(Error::Other("ELock from user enclave is not allowed"));
    //     }

    //     let enclave = unsafe { LinuxDriverEnclave::from_idx(user) };
    //     // we use the host satp to find the correct pma
    //     let host_satp = enclave.host_ctx.sregs.satp;

    //     if let Err(e) = self.change_pma_owner(
    //         vaddr..(vaddr + size),
    //         host_satp,
    //         enclave.id,
    //         Permission::RWX,
    //         |prop| prop.get_owner() == EnclaveId::HOST || prop.get_owner() == enclave.id,
    //     ) {
    //         log::error!("{e}");
    //         return Err(Error::Other(
    //             "ELock resource owned by non-host is not allowed",
    //         ));
    //     };

    //     Ok(())
    // }

    // pub fn free_mem(&self, user: EnclaveIdx, vaddr: usize, size: usize) -> Result<(), Error> {
    //     use enclave::EncListNode;

    //     if user == EnclaveIdx::HOST {
    //         return Err(Error::Other("Efree from host is not allowed"));
    //     }

    //     let node = EncListNode::from_idx(user);
    //     if node.is_user() {
    //         return Err(Error::Other("Efree from user enclave is not allowed"));
    //     }

    //     let enclave = unsafe { LinuxDriverEnclave::from_idx(user) };
    //     let host_satp = satp::read();

    //     if let Err(e) = self.change_pma_owner(
    //         vaddr..(vaddr + size),
    //         host_satp,
    //         enclave.id,
    //         Permission::RWX,
    //         |prop| prop.get_owner() == enclave.id,
    //     ) {
    //         log::error!("{e}");
    //         return Err(Error::Other(
    //             "Efree resource owned by other enclave is not allowed",
    //         ));
    //     };

    //     Ok(())
    // }

    #[allow(unused)]
    fn change_pma_owner(
        &self,
        region: Range<usize>,
        satp: satp::Satp,
        new_owner: EnclaveId,
        new_perm: impl Into<Permission> + Copy,
        mut check: impl FnMut(&PmaProp) -> bool,
    ) -> Result<(), Error> {
        let mut vaddr = region.start;
        while vaddr < region.end {
            let paddr = VirtAddr::from(vaddr)
                .translate(satp.ppn(), satp.mode(), &BarePtReader)
                .unwrap();
            let pma = self.pma_mgr.read().get_pma(paddr.0).unwrap();
            if !check(&pma.get_prop()) {
                return Err(Error::Other("Failed to change pma owner"));
            }
            // update pma ownership
            self.pma_mgr.write().insert_page(
                paddr,
                PmaProp::empty().owner(new_owner).permission(new_perm),
            );
            vaddr += 0x1000;
        }

        Ok(())
    }

    // pub fn ecall_request_read(
    //     &self,
    //     from: &mut LinuxDriverEnclave,
    //     head: &mut Head,
    //     tregs: &mut TrapRegs,
    // ) -> Result<(), Error> {
    //     use context::HartContext;

    //     let from = from.id;
    //     let to = EnclaveId::from(head.id as usize);

    //     // check the owner of each paddr
    //     // and update the owner to the receiver
    //     for paddr in head.iter_paddr() {
    //         let pma = self.pma_mgr.read().get_pma(paddr as usize).unwrap();
    //         if pma.get_prop().get_owner() != from {
    //             return Err(Error::Other("Failed to read from other enclave"));
    //         }
    //         self.pma_mgr
    //             .write()
    //             // the receiver is the owner of the paddr, and only allow to RW
    //             .insert_page(
    //                 paddr as usize,
    //                 PmaProp::empty().owner(to).permission(Permission::RW),
    //             );
    //     }

    //     // change the head page to the receiver, and only allow to read
    //     self.pma_mgr.write().insert_page(
    //         head as *const _ as usize,
    //         PmaProp::empty().owner(to).permission(Permission::R),
    //     );

    //     // stash the caller context
    //     let stash = unsafe { &mut *(head.stash as usize as *mut HartContext) };
    //     stash.save(tregs);

    //     // change id to the sender
    //     head.id = from.0 as u64;

    //     Ok(())
    // }

    // pub fn update_ctl_head(
    //     &self,
    //     from: &mut LinuxDriverEnclave,
    //     head: &mut Head,
    //     perm: impl Into<Permission> + Copy,
    //     tregs: &mut TrapRegs,
    // ) -> Result<(), Error> {
    //     use context::HartContext;

    //     let from = from.id;
    //     let to = EnclaveId::from(head.id as usize);

    //     // check the owner of each paddr
    //     // and update the owner to the receiver
    //     for paddr in head.iter_paddr() {
    //         let pma = self.pma_mgr.read().get_pma(paddr as usize).unwrap();
    //         if pma.get_prop().get_owner() != from {
    //             return Err(Error::Other("Failed to read from other enclave"));
    //         }
    //         self.pma_mgr
    //             .write()
    //             // the receiver is the owner of the paddr, and only allow to RW
    //             .insert_page(
    //                 paddr as usize,
    //                 PmaProp::empty().owner(to).permission(perm.into()),
    //             );
    //     }

    //     // change the head page to the receiver, and only allow to read
    //     self.pma_mgr.write().insert_page(
    //         head as *const _ as usize,
    //         PmaProp::empty().owner(to).permission(Permission::R),
    //     );

    //     // stash the caller context
    //     let stash = unsafe { &mut *(head.stash as usize as *mut HartContext) };
    //     stash.save(tregs);

    //     // change id to the sender
    //     head.id = from.0 as u64;

    //     Ok(())
    // }

    // pub fn ecall_finish_ctl(
    //     &self,
    //     caller: &mut LinuxUserEnclave,
    //     head: &mut Head,
    //     res: usize,
    //     tregs: &mut TrapRegs,
    // ) -> Result<TrapRegs, Error> {
    //     use context::HartContext;

    //     let from = caller.id;
    //     let to = EnclaveId::from(head.id as usize);

    //     // check the owner of each paddr
    //     // and update the owner back
    //     for paddr in head.iter_paddr() {
    //         let pma = self.pma_mgr.read().get_pma(paddr as usize).unwrap();
    //         if pma.get_prop().get_owner() != from {
    //             return Err(Error::Other("Failed to read from other enclave"));
    //         }
    //         self.pma_mgr.write().insert_page(
    //             paddr as usize,
    //             PmaProp::empty().owner(to).permission(Permission::RWX),
    //         );
    //     }

    //     // change the head page back
    //     self.pma_mgr.write().insert_page(
    //         head as *const _ as usize,
    //         PmaProp::empty().owner(to).permission(Permission::RWX),
    //     );

    //     // pop stash
    //     let stash = unsafe { &mut *(head.stash as usize as *mut HartContext) };
    //     let tregs = unsafe { stash.restore() };
    //     assert_eq!(tregs.a0, head as *const _ as usize);

    //     // change the result
    //     head.id = res as u64;

    //     Ok(tregs)
    // }
}
