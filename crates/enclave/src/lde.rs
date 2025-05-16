use core::{mem::offset_of, ops::Range};

use clint::ClintClient;
use context::HartContext;
use data_structure::linked_list::LinkedList;
use extension::Extension;
use hsm::Hsm;
use htee_channel::h2e::LdeInfo;
use htee_console::log;
use htee_device::device::Device;
use pma::{PhysMemAreaMgr, PmaProp};
use riscv::register::{Permission, mhartid, satp};
use vm::{allocator::FrameAllocator, page_table::PTEFlags, prelude::*};

use crate::{
    Builder, DEFAULT_BOOTARG_ADDR, Enclave, EnclaveData, EnclaveIdGenerator, EnclaveIdx,
    EnclaveType, Error, LinuxServiceEnclaveList, builder, node::EncListNode,
};

use super::{EnclaveId, HostInfo};

pub type LinuxDriverEnclave = Enclave<LinuxDriver>;
pub struct LinuxDriverEnclaveList(LinkedList<EnclaveType>);

impl LinuxDriverEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }
}

pub struct LinuxDriver {}

impl EnclaveData for LinuxDriver {
    const TYPE: EnclaveType = EnclaveType::Driver;
}

pub trait LdeClt {
    fn create_lde<A: FrameAllocator>(
        &self,
        info: &LdeInfo,
        enc_rt: usize,
        enc_bin: usize,
        allocator: A,
    ) -> Result<&'static mut LinuxDriverEnclave, Error>;
}

impl<T> LdeClt for T
where
    T: Extension<EnclaveIdGenerator>,
    T: Extension<PhysMemAreaMgr>,
    T: Extension<Hsm>,
    T: Extension<ClintClient>,
    T: Extension<Device>,
    T: Extension<LinuxServiceEnclaveList>,
{
    fn create_lde<A: FrameAllocator>(
        &self,
        load_info: &LdeInfo,
        enc_rt: usize,
        enc_bin: usize,
        allocator: A,
        // enc_bin: usize,
    ) -> Result<&'static mut LinuxDriverEnclave, Error> {
        log::debug!("{}", load_info);
        let host_satp = satp::read();
        log::debug!("host satp: {:#x}", host_satp.bits());

        let eid = self.view(|g: &EnclaveIdGenerator| g.fetch());

        let total_mem_size = load_info.mem.page_num * 0x1000;
        let enc_rt_start = enc_rt;
        let rt_size = align_up!(load_info.rt.size, 0x1000);
        let enc_bin_start = enc_bin;
        let bin_size = align_up!(load_info.bin.size, 0x1000);
        let stack_start = enc_rt_start - 0x1000;
        let boot_arg_addr = DEFAULT_BOOTARG_ADDR;

        log::info!("Creating linux driver enclave. Eid: {}", eid);

        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.update_pma_by_vaddr(
                load_info.mem.start.into(),
                total_mem_size,
                PmaProp::empty().owner(eid).permission(Permission::RWX),
                host_satp,
                |owner| owner == EnclaveId::HOST,
            );
        });

        // notify other harts to clear their pmp registers
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
        });

        self.view(|clint: &ClintClient| {
            clint.send_ipi_other_harts();
        });
        log::debug!("sync pmp registers");

        // alloc meta page and update meta page ownership
        let meta_page = VirtAddr(load_info.unused.start as usize + load_info.unused.size - 0x1000)
            .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
            .unwrap();
        log::debug!("meta page: {:#x}", meta_page.0);
        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.insert_page(
                meta_page,
                PmaProp::empty().owner(eid).permission(Permission::NONE),
            );
        });

        // get service enclave
        let lse = self
            .view(|list: &LinuxServiceEnclaveList| list.first())
            .ok_or_else(|| {
                log::error!("No service enclave found");
                Error::InvalidEnclaveId
            })?;

        let trampoline = VirtAddr(lse.data.rt_start)
            .translate(lse.normal_satp.ppn(), lse.normal_satp.mode(), &BarePtReader)
            .unwrap();

        // we firstly allocate the stack and args page
        let stack_ppn = allocator.alloc().unwrap();
        let args_ppn = allocator.alloc().unwrap();
        let serial_reg = self.view(|device: &Device| device.uart.get_reg());
        let serial_reg = serial_reg.start..(align_up!(serial_reg.end, 0x1000));

        let (lue, _) = builder(meta_page)
            .eid(eid)
            .prepare_vmm(allocator)
            .create_trampoline(trampoline)
            // runtime
            .map_lse(lse, VirtAddr(enc_rt_start))
            .map_host_pages(
                VirtPageNum::from_vaddr(load_info.rt.ptr),
                VirtPageNum::from_vaddr(enc_rt_start),
                rt_size / 0x1000,
                PTEFlags::rwx(),
            )
            // map stack
            .map_frames(
                stack_ppn,
                VirtPageNum::from_vaddr(stack_start),
                1,
                PTEFlags::rwx(),
            )
            // map args
            .map_frames(
                args_ppn,
                VirtPageNum::from_vaddr(boot_arg_addr),
                1,
                PTEFlags::rwx(),
            )
            // map binary
            .map_host_pages(
                VirtPageNum::from_vaddr(load_info.bin.ptr),
                VirtPageNum::from_vaddr(enc_bin_start),
                bin_size / 0x1000,
                PTEFlags::rwx(),
            )
            .map_frames(
                PhysPageNum::from_paddr(serial_reg.start),
                VirtPageNum::from_vaddr(serial_reg.start),
                (serial_reg.end - serial_reg.start) / 0x1000,
                PTEFlags::rwx(),
            )
            .finish();

        Ok(lue)
    }
}

// #[repr(C)]
// pub struct LinuxDriverEnclave {
//     pub id: EnclaveId,
//     pub host_info: HostInfo,
//     pub host_ctx: HartContext,
//     pub enclave_ctx: HartContext,
//     // the range of the vaddr that the lde matches
//     pub matches: Range<usize>,
//     pub name: [u8; 64],
//     pub tp: usize,
//     pub node: EncListNode,
// }

// impl LinuxDriverEnclave {
//     fn ll_node_offset() -> usize {
//         offset_of!(LinuxDriverEnclave, node)
//     }

//     pub fn uninit_at(addr: usize) -> &'static mut Self {
//         let enclave = unsafe { &mut *(addr as *mut Self) };
//         enclave
//     }

//     pub fn init_at(id: EnclaveId, addr: usize) -> &'static mut Self {
//         let enclave = unsafe { &mut *(addr as *mut Self) };
//         enclave.id = id;
//         enclave
//     }

//     pub unsafe fn from_node(node_ptr: *const EncListNode) -> &'static mut Self {
//         let offset = Self::ll_node_offset();
//         let ptr = (node_ptr as usize - offset) as *mut Self;
//         unsafe { &mut *ptr }
//     }

//     pub fn match_lde(&self, vaddr: usize) -> bool {
//         self.matches.contains(&vaddr)
//     }

//     pub fn idx(&self) -> EnclaveIdx {
//         self.node.idx()
//     }

//     /// Safety: The caller must ensure the enclave is valid LinuxDriverEnclave.
//     pub unsafe fn from_idx(idx: impl Into<EnclaveIdx>) -> &'static mut Self {
//         let idx: EnclaveIdx = idx.into();
//         let node = EncListNode::from_idx(idx);
//         unsafe { Self::from_node(node) }
//     }
// }
