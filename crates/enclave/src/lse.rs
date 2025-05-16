use core::ops::Range;

use clint::ClintClient;
use data_structure::linked_list::LinkedList;
use extension::Extension;
use hsm::Hsm;
use htee_channel::h2e::LseInfo;
use htee_console::log;
use perf::Profiler;
use pma::{PhysMemAreaMgr, PmaProp};
use riscv::register::{Permission, mhartid, satp};
use vm::{BarePtReader, Translate, VirtAddr, align_up};

use crate::{Enclave, EnclaveData, EnclaveId, EnclaveType, Error};

pub type LinuxServiceEnclave = Enclave<LinuxService>;

pub struct LinuxServiceEnclaveList(LinkedList<EnclaveType>);

impl LinuxServiceEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }

    pub fn first(&self) -> Option<&'static mut LinuxServiceEnclave> {
        self.0
            .iter()
            .next()
            .map(|ptr| unsafe { LinuxServiceEnclave::from_ptr(ptr) })
    }
}

pub trait LSEManager {
    fn create_lse(&self, arg0: usize) -> Result<&'static mut LinuxServiceEnclave, Error>;
}

impl<T> LSEManager for T
where
    T: Extension<LinuxServiceEnclaveList>,
    T: Extension<PhysMemAreaMgr>,
    T: Extension<ClintClient>,
    T: Extension<Hsm>,
    T: Extension<pmp::NwCache>,
{
    #[inline]
    fn create_lse(&self, arg0: usize) -> Result<&'static mut LinuxServiceEnclave, Error> {
        let host_satp = satp::read();
        let load_info = unsafe {
            let paddr = VirtAddr(arg0)
                .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
                .unwrap();

            &*(paddr.0 as *const LseInfo)
        };

        log::debug!("{}", load_info);
        log::debug!("host satp: {:#x}", host_satp.bits());

        let mem_start = load_info.mem.start as usize;
        let mem_size = load_info.mem.page_num * 0x1000;
        let rt_start = load_info.rt.ptr as usize;
        let rt_size = align_up!(load_info.rt.size, 0x1000);

        // we only measure binary in demo
        let mut ctx = md5::Context::new();
        let mut remain_size = rt_size;
        let mut addr = rt_start;

        let mut prof = perf::CycleProfiler::default();
        prof.start();

        while remain_size > 0 {
            let paddr = addr
                .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
                .unwrap();

            if remain_size >= 0x1000 {
                let bytes = unsafe { core::slice::from_raw_parts(paddr.0 as *const u8, 0x1000) };
                ctx.consume(bytes);
                remain_size -= 0x1000;
            } else {
                let bytes =
                    unsafe { core::slice::from_raw_parts(paddr.0 as *const u8, remain_size) };
                ctx.consume(bytes);
                remain_size = 0;
            }
            addr += 0x1000;
        }

        prof.stop();

        log::info!("rt md5sum: {:#x}", ctx.compute());
        log::info!(
            "measure cycle/page: {:#x}",
            prof.delta() / (rt_size / 0x1000)
        );

        // meta page should be at the start of the memory region
        debug_assert_eq!(mem_start, rt_start - 0x1000);
        debug_assert_eq!(mem_size, rt_size + 0x1000);

        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.update_pma_by_vaddr(
                load_info.mem.start.into(),
                load_info.mem.page_num * 0x1000,
                PmaProp::empty()
                    .owner(EnclaveId::EVERYONE)
                    .permission(Permission::RX),
                host_satp,
                |owner| owner == EnclaveId::HOST.into(),
            );
        });

        // use pmp::NwCacheExt;

        // self.update_nw_pmp_cache();

        self.view(|hsm: &Hsm| {
            for i in 0..hsm.num() {
                if i == mhartid::read() {
                    continue;
                }
                hsm.send_ops(i, hsm::HartStateOps {
                    clean_pmp: true,
                    ..hsm.recv_op(i)
                });
            }
            hsm.current().clean_pmp();
        });

        self.view(|clint: &ClintClient| clint.send_ipi_other_harts());
        log::debug!("sync pmp registers");

        // alloc meta page and update meta page ownership
        let meta_page = VirtAddr(mem_start)
            .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
            .unwrap();
        log::debug!("meta page: {:#x}", meta_page.0);

        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.insert_page(
                meta_page,
                PmaProp::empty()
                    .owner(EnclaveId::EVERYONE)
                    .permission(Permission::NONE),
            );
        });

        let enclave = LinuxServiceEnclave::create_at(meta_page.0);

        enclave.normal_region = mem_start..(mem_start + mem_size);
        enclave.normal_satp = host_satp;
        enclave.data.rt_start = rt_start;
        enclave.data.rt_size = rt_size;

        self.update(|enc: &mut LinuxServiceEnclaveList| {
            enc.0.push_node(&mut enclave.list.lock());
        });

        Ok(enclave)
    }
}

pub struct LinuxService {
    // pub paddr_region: Range<usize>,
    pub rt_start: usize,
    pub rt_size: usize,
}

impl EnclaveData for LinuxService {
    const TYPE: EnclaveType = EnclaveType::Service;
}

impl LinuxService {}
