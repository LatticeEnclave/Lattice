use core::{ptr::NonNull, sync::atomic::AtomicUsize};

use enclave::{Enclave, EnclaveId, EnclaveIdx, EnclaveType};
use heapless::Vec;
use hsm::{Hsm, MAX_HART_NUM};
use htee_console::log;
use htee_device::device::Device;
use pmp::{MAX_PMP_COUNT, PmpHelper};
use riscv::register::*;
use sbi::TrapRegs;
use spin::{Mutex, RwLock};
use trap_proxy::ProxyResult;
use vm::{
    allocator::FrameAllocator,
    mm::SV39,
    page_table::{BarePtReader, BarePtWriter, PTEFlags},
    prelude::*,
    vm::Sv39VmMgr,
};

use crate::{
    Error, PMP_COUNT, check_stack_overflow,
    ecall::{EcallError, EcallResult},
    enclave::{Builder, BuilderAllocator, EnclaveMgr, lse, lue, measure_data},
    helper,
};
use clint::ClintClient;
use pma::{Owner, PhysMemArea, PhysMemAreaMgr, PmaProp};

pub struct SecMonitor {
    pub pma_mgr: RwLock<PhysMemAreaMgr>,
    pub enc_mgr: EnclaveMgr,
    pub clint: ClintClient,
    pub hsm: Hsm,
    pub nw_cache: Mutex<pmp::NwCache>,
    pub nw_fault_num: AtomicUsize,

    pub device: Device,
}

impl SecMonitor {
    pub fn handle_trap(&self, regs: &mut TrapRegs) -> ProxyResult {
        let trap = mcause::read().cause();
        let res = match trap {
            mcause::Trap::Exception(e) => self.handle_exception(e, regs),
            mcause::Trap::Interrupt(i) => self.handle_interrupt(i, regs),
        };
        let cf = res.unwrap_or_else(|e| {
            log::error!("{e}");
            ProxyResult::Continue
        });

        cf
    }

    #[inline(always)]
    pub fn handle_interrupt(
        &self,
        interrupt: mcause::Interrupt,
        regs: &mut TrapRegs,
    ) -> Result<ProxyResult, Error> {
        let res = match interrupt {
            mcause::Interrupt::MachineSoft => self.handle_msoft_trap(regs),
            _ => ProxyResult::Continue,
        };

        Ok(res)
    }

    pub fn handle_msoft_trap(&self, _: &mut TrapRegs) -> ProxyResult {
        let hsm = &self.hsm;
        let op = hsm.take_op();
        if op.clean_pmp {
            if hsm.current().get_priv::<EnclaveIdx>().is_none() {
                // in normal world
                hsm.current().clean_pmp();
            }
            self.clint.reset_msip();
        }
        ProxyResult::Continue
    }

    pub fn handle_exception(
        &self,
        exception: mcause::Exception,
        regs: &mut TrapRegs,
    ) -> Result<ProxyResult, Error> {
        use sbi::ecall::SBI_EXT_HTEE_ENCLAVE;

        let res = match exception {
            mcause::Exception::IllegalInstruction => {
                if regs.a7 == SBI_EXT_HTEE_ENCLAVE && mtval::read() == 0 {
                    // unimp width
                    self.handle_ecall(regs, 0x2)
                } else {
                    ProxyResult::Continue
                }
            }
            mcause::Exception::LoadFault
            | mcause::Exception::StoreFault
            | mcause::Exception::InstructionFault => match self.handle_pmp_fault(regs) {
                Ok(_) => ProxyResult::Return,
                Err(e) => {
                    log::trace!("[SM] {e}");
                    // unsafe { regs.redirect_to_smode() };
                    ProxyResult::Continue
                }
            },
            mcause::Exception::Breakpoint => ProxyResult::Continue,
            // handle the ecall from S-mode ecall
            mcause::Exception::SupervisorEnvCall => {
                if regs.a7 == SBI_EXT_HTEE_ENCLAVE {
                    // ecall instruction length
                    self.handle_ecall(regs, 0x4)
                } else {
                    ProxyResult::Continue
                }
            }
            mcause::Exception::InstructionPageFault
            | mcause::Exception::LoadPageFault
            | mcause::Exception::StorePageFault => {
                log::warn!(
                    "hart {} get page fault:
mepc: {:#x}, mtval: {:#x}, mscratch: {:#x}, mcause: {:#x}
satp: {:#x}, stvec:{:#x}, scause: {:#x}
a0: {:#x}, a1: {:#x}, a2: {:#x}, ra:{:#x}, sp:{:#x}",
                    mhartid::read(),
                    regs.mepc,
                    mtval::read(),
                    mscratch::read(),
                    mcause::read().bits(),
                    satp::read().bits(),
                    stvec::read().bits(),
                    scause::read().bits(),
                    regs.a0,
                    regs.a1,
                    regs.a2,
                    regs.ra,
                    regs.sp,
                );
                panic!()
            }
            _ => ProxyResult::Continue,
        };

        Ok(res)
    }

    fn reset_harts_pmp(&self) {
        for i in 0..self.hsm.num() {
            if i == mhartid::read() {
                continue;
            }
            self.hsm.send_ops(i, hsm::HartStateOps {
                clean_pmp: true,
                ..self.hsm.recv_op(i)
            });
        }
        self.hsm.current().clean_pmp();
        self.clint.send_ipi_other_harts();
        log::debug!("cleaned harts pmp");
    }

    fn create_lue(&self, arg0: usize) -> Result<EcallResult, EcallError> {
        debug_assert_ne!(arg0, 0);
        let eid = self.enc_mgr.get_new_eid();
        let userargs = lue::get_args(arg0);

        // the entire memory
        self.pma_mgr.write().update_pma_by_vma(
            userargs.mem,
            PmaProp::empty().owner(eid).permission(Permission::RWX),
        );

        // meta page
        self.pma_mgr.write().update_pma_by_vma(
            VirtMemArea::default()
                .start(userargs.mem.start as usize)
                .size(PAGE_SIZE),
            PmaProp::empty().owner(eid).permission(Permission::NONE),
        );

        // share area
        self.pma_mgr.write().update_pma_by_vma(
            userargs.share,
            PmaProp::empty()
                .owner(EnclaveId::EVERYONE)
                .permission(Permission::RWX),
        );

        self.reset_harts_pmp();

        let lse = self.enc_mgr.get_lse(0).unwrap();
        let mut layout = lue::init_layout(&userargs, lse);

        let allocator = BuilderAllocator::new(
            VirtMemArea::default()
                .start(userargs.unused.start as usize)
                .size(userargs.unused.size - 0x1000),
        );

        let mut builder = Builder {
            vmm: Sv39VmMgr::new(
                allocator.alloc().unwrap(),
                BarePtWriter,
                allocator,
                satp::read().asid(),
                SV39,
            ),
        };

        let enc = builder.create_lue(&userargs, eid);
        enc.nw_vma = userargs.mem;

        // map trampoline
        let trampoline = builder.create_trampoline(lse.data.trampoline);
        layout.trampoline = trampoline;

        // map lse
        builder.map_vma(lse.data.rt, layout.rt);

        // map stack
        builder.alloc_vma(layout.stack).unwrap();
        enc.data.enc_ctx.tregs.sp = layout.stack.start + layout.stack.size;

        // map args
        let bootargs_vma = builder.alloc_vma(layout.bootargs).unwrap();
        enc.data.enc_ctx.tregs.a1 = bootargs_vma.start;

        // map share
        builder.map_vma(userargs.share, layout.share);
        // map binary
        builder.map_vma(userargs.binary, layout.binary);
        // map serial
        builder.map_frames(
            PhysPageNum::from_paddr(self.device.uart.get_reg().start),
            VirtMemArea::default()
                .start(self.device.uart.get_reg().start)
                .size(align_up!(self.device.uart.get_reg().len(), PAGE_SIZE))
                .flags(PTEFlags::rw().dirty().accessed()),
        );

        let (head, free_size) = builder.collect_unused();

        lue::create_bootargs(
            bootargs_vma,
            userargs.mem.size(userargs.mem.size - 0x1000),
            layout,
            userargs,
            head,
            free_size,
            self.device.clone(),
        );

        self.enc_mgr.push_lue(enc);

        Ok(EcallResult::ret().retval(eid.0))
    }

    fn create_lse(&self, arg0: usize) -> Result<EcallResult, EcallError> {
        let eid = self.enc_mgr.get_new_eid();
        log::debug!("eid: {eid}");
        let userargs = lse::get_user_args(arg0);

        debug_assert!(aligned!(userargs.rt.start, PAGE_SIZE));
        debug_assert_eq!(userargs.mem.start, userargs.rt.start - 0x1000);
        debug_assert_eq!(
            userargs.mem.size,
            align_up!(userargs.rt.size, PAGE_SIZE) + 0x1000
        );

        self.pma_mgr.write().update_pma_by_vma(
            userargs.rt,
            PmaProp::empty()
                .owner(Owner::EVERYONE)
                .permission(Permission::RX),
        );

        self.pma_mgr.write().update_pma_by_vma(
            userargs.mem.size(PAGE_SIZE),
            PmaProp::empty()
                .owner(Owner::EVERYONE)
                .permission(Permission::NONE),
        );
        self.reset_harts_pmp();

        let md5 = measure_data(userargs.rt);
        log::debug!("md5: {:#x}", md5);

        let allocator = BuilderAllocator::new(
            VirtMemArea::default()
                .start(userargs.unused.start as usize)
                .size(userargs.unused.size),
        );

        let mut builder = Builder {
            vmm: Sv39VmMgr::new(
                PhysPageNum(0),
                BarePtWriter,
                allocator,
                satp::read().asid(),
                SV39,
            ),
        };

        let enc = builder.create_lse(&userargs, eid);
        enc.nw_vma = userargs.mem;
        enc.data.rt = userargs.rt;
        enc.data.trampoline = userargs.rt.size(PAGE_SIZE);
        self.enc_mgr.push_lse(enc);

        Ok(EcallResult::ret().retval(eid.0))
    }

    fn create_enclave(&self, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        const USER_ENC: usize = EnclaveType::User as usize;
        const SER_ENC: usize = EnclaveType::Service as usize;

        match regs.a1 {
            USER_ENC => self.create_lue(regs.a0),
            SER_ENC => self.create_lse(regs.a0),
            _ => panic!("unknown enclave type"),
        }
    }

    fn destory_enclave(&self, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        // SAFETY: Enclave<()> is safe
        let enc = self
            .hsm
            .current()
            .get_priv::<EnclaveIdx>()
            .unwrap()
            .as_enc();
        let owner = enc.id();

        enc.print_records();

        if let Some(enc) = enc.as_lue() {
            log::info!("enclave pause num: {}", enc.data.pause_num);
        }

        log::info!("[SM] Cleaning enclave {}", owner);

        self.enc_mgr.rm_lue(owner);
        *regs = unsafe { enc.nw_ctx.restore() };

        // clean memory content
        // SAFETY: it is safe to clean the enclave memory content by using host satp.
        // 因为，如果操作系统去掉了某个页的映射，那SM就不会复原这个页的所有者，这会导致这个页永远也无法被访问。
        for vpn in enc.nw_vma.iter_vpn() {
            let paddr = vpn
                .translate(enc.nw_vma.satp.ppn(), enc.nw_vma.satp.mode(), &BarePtReader)
                .unwrap();
            let pma = self.pma_mgr.read().get_pma(paddr).unwrap();
            let pma_owner = pma.get_prop().get_owner();
            // we still need to check the owner of the page, avoiding cleaning the page that is not owned by the enclave
            if pma_owner == owner {
                unsafe { clean_page_content(paddr.0 as usize) };
                self.pma_mgr.write().insert_page(
                    paddr,
                    PmaProp::empty()
                        .owner(EnclaveId::HOST.0)
                        .permission(Permission::RWX),
                );
            } else if pma_owner == EnclaveId::EVERYONE {
                self.pma_mgr.write().insert_page(
                    paddr,
                    PmaProp::empty()
                        .owner(EnclaveId::HOST.0)
                        .permission(Permission::RWX),
                );
            } else {
                log::error!("cleaning pma {pma} owned by {}", pma_owner);
                panic!(
                    "[SM] Invalid pma owner in cleaning enclave. The correct owner should be {owner} or {}, but got {}.",
                    EnclaveId::EVERYONE,
                    pma_owner
                );
            }
        }

        self.hsm.current().clear_priv();
        log::info!("[SM] Enclave {} cleaned", owner);

        Ok(EcallResult::ret().retval(0).fixed_epc())
    }

    fn launch_enclave(&self, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        let eid = EnclaveId::from(regs.a0);
        log::debug!("Launch enclave. Id: #{}", eid);
        #[allow(unused_assignments)]
        let mut args = (0, 0);
        #[allow(unused_assignments)]
        let mut addr = 0;
        #[allow(unused_assignments)]
        let mut sp = 0;

        if let Some(enc) = self.enc_mgr.get_lue(eid) {
            args = lue::prepare_launch(enc, regs);
            addr = enclave::DEFAULT_RT_START;
            sp = enc.data.enc_ctx.tregs.sp;
            self.hsm.current().set_priv(enc.idx());
            log::debug!("Set enclave idx #{}", enc.idx());
        } else if let Some(_) = self.enc_mgr.get_lse(eid) {
            panic!("Unsupported yet")
        } else {
            panic!("Enclave not found")
        }

        self.hsm.current().clean_pmp();
        unsafe { stvec::write(0, stvec::TrapMode::Direct) };

        log::debug!("lue arg0: {:#x}, arg1: {:#x}", args.0, args.1);
        log::debug!("enclave entry: {addr:#x}");
        log::debug!("entry satp: {:#x}", satp::read().bits());

        unsafe {
            self.hsm.mret(
                addr,
                mstatus::MPP::Supervisor,
                args.0,
                args.1,
                sp,
                satp::read().bits(),
            )
        }
    }

    fn resume_enclave(&self, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        // todo!()
        let eid = EnclaveId::from(regs.a0);
        // unimp length
        regs.mepc += 0x2;

        log::debug!("hart {} resuming enclave #{eid}", mhartid::read());
        let enc = self
            .enc_mgr
            .get_lue(eid)
            .ok_or(enclave::Error::InvalidEnclaveId)
            .map_err(|e| {
                log::error!("{e}");
                EcallError::code(e as usize)
            })?;

        // set current enclave
        log::debug!("Set current enclave to #{eid}, idx: {}", enc.idx());

        self.hsm.current().clean_pmp();
        self.hsm.current().set_priv(enc.idx());
        // });

        // restore the pmp status
        enc.data.pmp_cache.restore();
        log::debug!("restore pmp entires");

        // save new context
        enc.nw_ctx.save(&regs);

        debug_assert_ne!(enc.data.enc_ctx.sregs.satp, 0);
        debug_assert_ne!(enc.data.enc_ctx.sregs.satp, satp::read().bits());

        log::debug!("restore enclave context:");
        log::debug!("satp: {:#x}", enc.data.enc_ctx.sregs.satp);
        log::debug!("sscratch: {:#x}", enc.data.enc_ctx.sregs.sscratch);
        log::debug!("sstatus: {:#x}", enc.data.enc_ctx.sregs.sstatus);
        log::debug!("stvec: {:#x}", enc.data.enc_ctx.sregs.stvec);
        log::debug!("sepc: {:#x}", enc.data.enc_ctx.sregs.sepc);
        log::debug!("scaues: {:#x}", enc.data.enc_ctx.sregs.scaues);
        log::debug!("stval: {:#x}", enc.data.enc_ctx.sregs.stval);
        // SAFETY: It is ready to switch context
        *regs = unsafe { enc.data.enc_ctx.restore() };
        riscv::asm::sfence_vma_all();

        enc.data.switch_cycle.end();

        // let cycle_finish = riscv::register::cycle::read();
        // log::info!("cycle in resume enclave: {:#x}", cycle_finish - cycle_start);
        // unsafe { riscv::register::mcountinhibit::set_cy() };

        Ok(EcallResult::ret().fixed_epc())
    }

    fn pause_enclave(&self, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        // unsafe { riscv::register::mcountinhibit::clear_cy() };
        // let cycle_start = riscv::register::cycle::read();

        log::debug!("hart {} pausing enclave", mhartid::read());
        // SAFETY: It is safe to convert EnclaveIdx to Enclave<()>
        let enc = self
            .hsm
            .current()
            .get_priv::<EnclaveIdx>()
            .map(|idx| {
                log::debug!("current enclave idx: {}", idx);
                idx
            })
            .unwrap()
            .as_enc();

        match enc.get_type() {
            EnclaveType::User => {
                let enc = enc.as_lue().unwrap();
                self.hsm.current().clear_priv();
                lue::pause(enc, regs)
                    .map(|_| {
                        log::debug!("pause enclave, return to {:#x}", regs.mepc);
                        EcallResult::ret().retval(regs.a1).fixed_epc()
                    })
                    .map_err(|e| {
                        log::error!("pause enclave failed: {}", e);
                        EcallError::code(e as usize)
                    })
            }
            EnclaveType::Service => {
                log::error!("service enclave cannot be paused");
                Err(EcallError::code(0x1))
            }
            _ => {
                log::error!("unsupported enclave type");
                Err(EcallError::code(0x1))
            }
        }
    }

    #[inline]
    pub fn handle_ecall(&self, regs: &mut TrapRegs, offset: usize) -> ProxyResult {
        use crate::ecall::*;
        use crate::enclave::*;

        #[cfg(debug_assertions)]
        check_stack_overflow();

        let funcid = regs.a6;
        let extid = regs.a7;

        let res = EcallHandler::new(CREATE_ENC, EXT_ID, SecMonitor::create_enclave)
            .add_ecall(LAUNCH_ENC, EXT_ID, SecMonitor::launch_enclave)
            .add_ecall(EXIT_ENC, EXT_ID, SecMonitor::destory_enclave)
            .add_ecall(DESTROY_ENC, EXT_ID, SecMonitor::destory_enclave)
            .add_ecall(RESUME_ENC, EXT_ID, SecMonitor::resume_enclave)
            .add_ecall(PAUSE_ENC, EXT_ID, SecMonitor::pause_enclave)
            .call(self, regs);

        let res = match res {
            Ok(r) => {
                if !r.fixed_epc {
                    unsafe {
                        regs.fix_mepc(offset);
                    }
                }
                regs.a0 = 0;
                regs.a1 = r.retval;
                // we will clean a6, a7
                regs.a6 = 0;
                regs.a7 = 0;
                r.proxy
            }
            Err(e) => match e {
                EcallError::UnsupportedFunc => {
                    // log::error!("Unsupported function. We will passing to SBI");
                    ProxyResult::Continue
                }
                EcallError::EcallRuntime(code) => {
                    log::error!(
                        "Handling ecall func failed in hart {:#x}: funcid {:#x}, ext: {:#x}, code: {}",
                        mhartid::read(),
                        funcid,
                        extid,
                        code
                    );
                    regs.a0 = code;
                    regs.a1 = 0;
                    regs.a6 = 0;
                    regs.a7 = 0;
                    unsafe {
                        regs.fix_mepc(offset);
                    }
                    ProxyResult::Return
                }
            },
        };

        if let ProxyResult::Return = res {
            log::debug!(
                "hart #{} handle ecall finished, return to {:#x}",
                mhartid::read(),
                regs.mepc
            );
        }

        res
    }

    #[inline]
    pub fn handle_pmp_fault(&self, regs: &mut TrapRegs) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        check_stack_overflow();

        let buf = unsafe { self.hsm.current().pmp_buf.as_mut() };
        buf.clear();

        self.handle_all_fault(regs, buf)?;

        Ok(())
    }

    #[inline]
    fn handle_all_fault(
        &self,
        regs: &mut TrapRegs,
        buf: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
    ) -> Result<(), Error> {
        use helper::*;
        use riscv::register::{mepc, mstatus, satp};
        use vm::mm::*;

        // unsafe { riscv::register::mcountinhibit::clear_cy() };
        // let cycle_start = riscv::register::cycle::read();

        let hartid = mhartid::read();
        log::trace!("hart #{hartid} handle pmp fault at {:#x}", regs.mepc);
        let satp = satp::read();
        let mepc = mepc::read();
        let mtval = mtval::read();
        let mcause = mcause::read().bits();

        let idx = self.hsm.current().get_priv::<EnclaveIdx>();
        let enc: Option<&'static mut Enclave<()>> =
            idx.map(|idx| unsafe { Enclave::from_ptr(idx) });
        let eid = enc.map(|enc| enc.id()).unwrap_or(EnclaveId::HOST);
        if let Some(idx) = idx {
            log::trace!("hart {hartid} enclave idx: {idx}");
        } else {
            #[cfg(debug_assertions)]
            {
                let num = self
                    .nw_fault_num
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                log::trace!("nw num: {}", num);
            }

            log::trace!("hart {hartid} Normal world");
        }

        log::trace!(
            "hart {hartid}, satp: {:#x}, mepc: {mepc:#x}, mtval: {mtval:#x}, mcause: {mcause:#x}",
            satp.bits()
        );
        log::trace!(
            "sstatus: {:#x}, stvec: {:#x}, sepc: {:#x}, scause: {:#x}",
            sstatus::read().bits(),
            stvec::read().bits(),
            sepc::read(),
            scause::read().bits()
        );

        // let current_pmas = sm().get_current_pmas();

        // log::trace!("current pma:");
        // current_pmas.iter().for_each(|p| log::trace!("{}", p));

        let mpp = mstatus::read().mpp();

        if mpp == mstatus::MPP::Machine {
            log::error!("mpp is Machine");
            log::error!("mepc: {mepc:#x}");
            log::error!("mtval: {mtval:#x}");
            log::error!("hart id: {}", mhartid::read());
            panic!();
        }

        match satp.mode() {
            satp::Mode::Bare => {
                log::trace!("Bare mode");
                pmas_on_paddr(&self.pma_mgr.read(), mepc, mtval, buf).unwrap()
            }
            satp::Mode::Sv39 => {
                log::trace!("SV39 mode");
                pmas_req_vaddr(&self.pma_mgr.read(), mepc, mtval, satp.ppn(), SV39, buf)?
            }
            satp::Mode::Sv48 => {
                log::trace!("SV48 mode");
                pmas_req_vaddr(&self.pma_mgr.read(), mepc, mtval, satp.ppn(), SV48, buf)?
            }
            satp::Mode::Sv57 => todo!(),
            satp::Mode::Sv64 => todo!(),
        };

        log::trace!("required pma:");
        buf.iter()
            .for_each(|p| log::trace!("{:#x} => {}", p.addr, p.pma));

        for p in buf.iter() {
            if !p
                .pma
                .check_owner(|owner| owner == eid || owner == EnclaveId::EVERYONE)
            {
                log::error!("Enclave #{} is not allowed to access {}", eid, p.pma);
                log::error!("The region owned by #{}", p.pma.get_prop().get_owner());
                log::error!("mepc: {mepc:#x}");
                log::error!("mtval: {mtval:#x}");
                log::error!("hart id: {}", mhartid::read());
                panic!();
            }
        }

        update_pmp_by_pmas(buf, self.iter_current_pma());

        log::trace!("Updated pmp registers");

        if idx.is_none() {
            // update normal world cache
            self.update_nw_pmp_cache();
        }

        let _ = idx
            .map(|idx| unsafe { Enclave::<()>::from_ptr(idx) })
            .map(|enc| enc.pmp_record.finish_handle());

        // let cycle_finish = riscv::register::cycle::read();
        // log::info!(
        //     "cycle in handling pmp fault: {:#x}",
        //     cycle_finish - cycle_start
        // );
        // unsafe { riscv::register::mcountinhibit::set_cy() };

        Ok(())
    }

    fn update_nw_pmp_cache(&self) {
        // self.update(|nw_cache: &mut NwCache| {
        self.nw_cache.lock().clear();
        for pmp in pmp::iter_hps() {
            self.nw_cache.lock().push(pmp);
        }
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
}

/// The caller should never use the content again
unsafe fn clean_page_content(page: usize) {
    let addr = page as *mut u8;
    for i in 0..4096 {
        unsafe { addr.add(i).write(0) };
    }
}
