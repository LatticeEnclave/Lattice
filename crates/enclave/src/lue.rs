use clint::ClintClient;
use data_structure::linked_list::LinkedList;
use extension::Extension;
use hsm::Hsm;
use htee_channel::{e2r::LueBootArgs, h2e::LueInfo};
use htee_console::log;
use htee_device::device::Device;
use perf::Profiler;
use pma::{PhysMemAreaMgr, PmaProp};
use riscv::register::{Permission, medeleg, mhartid, mstatus, satp, stvec};
use sbi::TrapRegs;

use context::HartContext;
use vm::{
    BarePtReader, PhysAddr, PhysPageNum, Translate, VirtAddr, align_up, allocator::FrameAllocator,
    page_table::PTEFlags, vm::VirtPageNum,
};

use crate::{
    Builder, DEFAULT_BOOTARG_ADDR, DEFAULT_RT_START, Enclave, EnclaveData, EnclaveIdGenerator,
    EnclaveType, Error, LinuxServiceEnclaveList, allocator::BuilderAllocator, builder,
    link_remain_frame,
};

use super::EnclaveId;

pub type LinuxUserEnclave = Enclave<LinuxUser>;

pub struct LinuxUserEnclaveList(LinkedList<EnclaveType>);

impl LinuxUserEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }

    pub fn get(&self, eid: EnclaveId) -> Option<&'static mut LinuxUserEnclave> {
        self.0.iter().find_map(|ptr| {
            // Safety: the node is pushed by &mut LinuxUserEnclave, thus it is valid
            let lue = unsafe { LinuxUserEnclave::from_ptr(ptr) };
            if lue.id == eid { Some(lue) } else { None }
        })
    }

    pub fn push(&mut self, lue: &'static mut LinuxUserEnclave) {
        debug_assert_eq!(lue.get_type(), EnclaveType::User);
        self.0.push_node(&mut lue.list.lock());
    }

    pub fn remove(&mut self, eid: EnclaveId) -> Option<&'static mut LinuxUserEnclave> {
        let enc = self
            .get(eid)
            .and_then(|enc| self.0.rm_node(&mut enc.list.lock()))
            .map(|node| unsafe { Enclave::from_ptr(node) })?;

        log::debug!("remaining enclaves:");
        for e in self.0.iter() {
            let eid = unsafe { Enclave::<()>::from_ptr(e).id };
            log::debug!("{eid}");
        }

        Some(enc)
    }
}

pub trait LueCtl {
    fn create_lue(
        &self,
        info: &LueInfo,
        enc_rt: *const u8,
        enc_bin: *const u8,
        // allocator: A,
    ) -> Result<(&'static mut LinuxUserEnclave, &'static mut LueBootArgs), Error>;

    fn launch_lue(&self, addr: usize, enc: &mut LinuxUserEnclave) -> Result<!, Error>;

    fn pause_lue(&self, enc: &mut LinuxUserEnclave, regs: &mut TrapRegs) -> Result<(), Error>;
}

impl<T> LueCtl for T
where
    T: Extension<LinuxUserEnclaveList>,
    T: Extension<LinuxServiceEnclaveList>,
    T: Extension<EnclaveIdGenerator>,
    T: Extension<PhysMemAreaMgr>,
    T: Extension<Hsm>,
    T: Extension<ClintClient>,
    T: Extension<Device>,
    T: Extension<pmp::NwCache>,
{
    fn pause_lue(&self, enc: &mut LinuxUserEnclave, regs: &mut TrapRegs) -> Result<(), Error> {
        enc.data.switch_cycle.begin();

        enc.data.pause_num += 1;

        log::debug!("Pausing lue #{}", enc.id.0);
        regs.mepc += 0x4;

        let rc = regs.a0;
        enc.data.enc_ctx.save(regs);
        log::debug!("saved enclave context:");
        log::debug!("satp: {:#x}", enc.data.enc_ctx.sregs.satp);
        log::debug!("sscratch: {:#x}", enc.data.enc_ctx.sregs.sscratch);
        log::debug!("sstatus: {:#x}", enc.data.enc_ctx.sregs.sstatus);
        log::debug!("stvec: {:#x}", enc.data.enc_ctx.sregs.stvec);
        log::debug!("sepc: {:#x}", enc.data.enc_ctx.sregs.sepc);
        log::debug!("scaues: {:#x}", enc.data.enc_ctx.sregs.scaues);
        log::debug!("stval: {:#x}", enc.data.enc_ctx.sregs.stval);

        // save current pmp
        enc.data.pmp_cache.dump();
        log::debug!("dumpped pmp entires");

        *regs = unsafe { enc.normal_ctx.restore() };
        regs.a0 = 0;
        regs.a1 = rc;
        // regs.a0 = eid.into();
        self.view(|hsm: &Hsm| {
            // log::debug!("cleaning pmp");
            // hsm.current().clean_pmp();
            log::debug!("setting priv to null");
            hsm.current().clear_priv();
        });

        enc.data.switch_cycle.end();
        Ok(())
    }

    fn launch_lue(&self, addr: usize, enc: &mut LinuxUserEnclave) -> Result<!, Error> {
        let arg0 = enc.data.enc_ctx.tregs.a0;
        let arg1 = enc.data.enc_ctx.tregs.a1;

        log::info!("Enclave #{} launching", enc.id.0);
        self.view(|hsm: &Hsm| {
            hsm.current().clean_pmp();
            log::debug!("Set enclave idx #{}", enc.idx());
            hsm.current().set_priv(enc.idx());
            debug_assert_ne!(enc.data.enc_ctx.sregs.satp, 0);
            debug_assert_ne!(enc.data.enc_ctx.sregs.satp, satp::read().bits());
            satp::write(enc.data.enc_ctx.sregs.satp);
            unsafe { stvec::write(0, stvec::TrapMode::Direct) };
            // for pte in vm::VAddrTranslator::new(
            //     VirtPageNum::from_vaddr(addr),
            //     satp::read().ppn().into(),
            //     &BarePtReader,
            //     vm::mm::SV39,
            // )
            // .iter_pte()
            // {
            //     log::debug!("pte: {:#x}", pte.bits);
            // }
            // satp::write(0);
            log::debug!("lue arg0: {arg0}, arg1: {arg1:#x}");
            log::debug!("enclave entry: {addr:#x}");
            // log::debug!("entry paddr: {:#x}", paddr.0);
            log::debug!("entry satp: {:#x}", satp::read().bits());
            // log::debug!("medeleg: {:#x}", medeleg::read().bits());
            // unsafe {
            //     medeleg::clear_load_page_fault();
            //     medeleg::clear_instruction_page_fault();
            //     medeleg::clear_store_page_fault();
            // }
            // log::debug!("medeleg: {:#x}", medeleg::read().bits());
            unsafe {
                hsm.mret(
                    addr,
                    mstatus::MPP::Supervisor,
                    arg0,
                    arg1,
                    enc.data.enc_ctx.tregs.sp,
                    enc.data.enc_ctx.sregs.satp,
                )
            }
        })
        // unsafe {
        //     hart::current().enter_enclave(
        //         enclave.idx(),
        //         RT_VADDR_START,
        //         arg0,
        //         arg1,
        //         RT_VADDR_START,
        //         enclave.enclave_ctx.sregs.satp,
        //     )
        // }
    }

    fn create_lue(
        &self,
        info: &LueInfo,
        enc_rt: *const u8,
        enc_bin: *const u8,
        // allocator: A,
    ) -> Result<(&'static mut LinuxUserEnclave, &'static mut LueBootArgs), Error> {
        log::debug!("{}", info);

        let host_satp = satp::read();
        log::debug!("host satp: {:#x}", host_satp.bits());

        let eid = self.view(|g: &EnclaveIdGenerator| g.fetch());
        let mem_start = info.mem.start as usize;
        let total_mem_size = info.mem.page_num * 0x1000;
        let enc_rt_start = enc_rt as usize;
        let rt_size = align_up!(info.rt.size, 0x1000);
        let enc_bin_start = enc_bin as usize;
        let bin_size = align_up!(info.bin.size, 0x1000);
        let enc_share_start = enc_bin_start + bin_size;
        let share_start = info.shared.ptr as usize;
        let share_size = align_up!(info.shared.size, 0x1000);
        let stack_start = enc_rt_start - 0x1000;
        let boot_arg_addr = DEFAULT_BOOTARG_ADDR;
        // let enc_mods_start = enc_bin_start + bin_size;

        let mut prof = perf::CycleProfiler::default();
        prof.start();

        // we only measure binary in demo
        let mut ctx = md5::Context::new();
        let mut remain_size = info.bin.size;
        let mut addr = info.bin.ptr as usize;

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

        // prof.stop();

        log::info!("binary md5sum: {:#x}", ctx.compute());
        // log::info!(
        //     "measure cycle/page: {:#x}",
        //     prof.delta() / (remain_size / 0x1000)
        // );

        // total memory size should be the sum of all the memory regions
        debug_assert_eq!(
            total_mem_size,
            rt_size + bin_size + share_size + info.unused.size + 0x1000
        );

        // meta page should be at the start of the memory region
        debug_assert_eq!(mem_start, info.rt.ptr as usize - 0x1000);

        log::info!("Creating linux user enclave. Eid: {}", eid);

        if self.view(|mgr: &PhysMemAreaMgr| {
            mgr.iter_pma().any(|pma| pma.get_prop().get_owner() == eid)
        }) {
            log::error!("Existing pma for enclave {}", eid);
            panic!()
        }

        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.update_pma_by_vaddr(
                VirtAddr(mem_start),
                total_mem_size,
                PmaProp::empty().owner(eid).permission(Permission::RWX),
                host_satp,
                |owner| {
                    // owner
                    // log::debug!("")
                    if owner == EnclaveId::HOST {
                        true
                    } else {
                        log::debug!("owner: {}", owner);
                        false
                    }
                },
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
            hsm.current().clean_pmp();
        });

        // use pmp::NwCacheExt;

        // self.update_nw_pmp_cache();

        self.view(|clint: &ClintClient| clint.send_ipi_other_harts());
        log::debug!("cleaned harts pmp");

        // alloc meta page and update meta page ownership
        let meta_page = VirtAddr(mem_start)
            .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
            .unwrap();
        log::debug!("meta page: {:#x}", meta_page.0);
        self.update(|mgr: &mut PhysMemAreaMgr| {
            mgr.insert_page(
                meta_page,
                PmaProp::empty().owner(eid).permission(Permission::NONE),
            );
        });

        log::debug!("{eid} owned pma:");
        let pma_num = self.view(|mgr: &PhysMemAreaMgr| {
            mgr.iter_pma()
                .filter(|pma| {
                    if pma.get_prop().get_owner() == eid
                        && pma.get_prop().get_owner_perm() == Permission::RWX
                    {
                        log::debug!("{}", pma);
                        true
                    } else {
                        false
                    }
                })
                .count()
        });
        log::info!("{eid} own {pma_num} PMAs");

        // change shared page owner
        self.update(|mgr: &mut PhysMemAreaMgr| {
            let mut addr = share_start;
            while addr < share_start + share_size {
                let page = VirtAddr(addr)
                    .translate(host_satp.ppn(), host_satp.mode(), &BarePtReader)
                    .unwrap();
                mgr.insert_page(
                    page,
                    PmaProp::empty()
                        .owner(EnclaveId::EVERYONE)
                        .permission(Permission::RWX),
                );

                addr += 0x1000;
            }
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

        log::debug!("lse entry data: {:#x}", unsafe {
            *(trampoline.0 as *const u64)
        });

        let allocator = BuilderAllocator::new(
            host_satp.ppn(),
            host_satp.mode(),
            info.unused.start as usize,
            info.unused.size - 0x1000,
        );

        // we firstly allocate the stack and args page
        let stack_ppn = allocator.alloc().unwrap();
        let args_ppn = allocator.alloc().unwrap();
        let serial_reg = self.view(|device: &Device| device.uart.get_reg());
        let serial_reg = serial_reg.start..(align_up!(serial_reg.end, 0x1000));

        let (lue, allocator): (&mut Enclave<LinuxUser>, _) = builder(meta_page)
            .eid(eid)
            .prepare_vmm(allocator)
            .create_trampoline(trampoline)
            // runtime
            .map_lse(lse, VirtAddr(enc_rt_start))
            // map stack
            .map_frames(
                stack_ppn,
                VirtPageNum::from_vaddr(stack_start),
                1,
                PTEFlags::rw().dirty().accessed(),
            )
            // map args
            .map_frames(
                args_ppn,
                VirtPageNum::from_vaddr(boot_arg_addr),
                1,
                PTEFlags::rw().dirty().accessed(),
            )
            // map shared pages
            .map_host_pages(
                VirtPageNum::from_vaddr(info.shared.ptr),
                VirtPageNum::from_vaddr(enc_share_start),
                share_size / 0x1000,
                PTEFlags::rw().dirty().accessed(),
            )
            // map binary
            .map_host_pages(
                VirtPageNum::from_vaddr(info.bin.ptr),
                VirtPageNum::from_vaddr(enc_bin_start),
                bin_size / 0x1000,
                PTEFlags::rw().dirty().accessed(),
            )
            .map_frames(
                PhysPageNum::from_paddr(serial_reg.start),
                VirtPageNum::from_vaddr(serial_reg.start),
                (serial_reg.end - serial_reg.start) / 0x1000,
                PTEFlags::rw().dirty().accessed(),
            )
            .finish();

        lue.normal_region = mem_start..(mem_start + total_mem_size);
        lue.data.rt_entry = trampoline.0;
        lue.data.switch_cycle = perf::CycleRecord::default();
        lue.data.pause_num = 0;

        let (head, free_size) = link_remain_frame(allocator.start(), allocator.size(), host_satp);

        use htee_channel::enclave::runtime::*;
        let args = unsafe { &mut *(PhysAddr::from_ppn(args_ppn).0 as *mut LueBootArgs) };
        *args = LueBootArgs {
            mem: MemArg {
                total_size: total_mem_size - 0x1000,
            },
            mods: ModArg {
                start_vaddr: 0,
                num: 0,
            },
            tp: TpArg { addr: lue.tp },
            bin: BinArg {
                start: enc_bin as usize,
                size: bin_size,
            },
            shared: SharedArg {
                enc_vaddr: enc_share_start,
                host_vaddr: share_start,
                size: share_size,
            },
            unmapped: UnmappedArg {
                head,
                size: free_size,
            },
            device: self.view(|device: &Device| device.clone()),
        };

        lue.data.enc_ctx.tregs.a0 = 0;
        lue.data.enc_ctx.tregs.a1 = boot_arg_addr;
        lue.data.enc_ctx.tregs.sp = enc_rt_start;

        log::info!("cycle/page: {}", prof.delta() / (total_mem_size / 0x1000));

        Ok((lue, args))
    }
}

pub struct LinuxUser {
    pub enc_ctx: HartContext,
    pub rt_entry: usize,
    pub pmp_cache: pmp::Cache,

    pub pause_num: usize,
    pub switch_cycle: perf::CycleRecord,
}

impl EnclaveData for LinuxUser {
    const TYPE: EnclaveType = EnclaveType::User;
}
