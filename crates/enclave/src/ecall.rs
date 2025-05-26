use core::ops::Range;

use clint::ClintClient;
use context::SupervisorRegs;
use extension::{Ecall, EcallResult, Extension, id_range};
use hsm::Hsm;
use channel::h2e::LueInfo;
use console::log;
use device::device::Device;
use pma::{PhysMemAreaMgr, PmaProp};
use riscv::interrupt::free;
use riscv::register::{Permission, mhartid, satp, scause, sepc};
use sbi::TrapRegs;
use sbi::profiling::get_instret;
use vm::allocator::FrameAllocator;
use vm::consts::PAGE_SIZE;
use vm::prelude::*;

use crate::allocator::BuilderAllocator;
use crate::lue::LinuxUser;
use crate::{
    DEFAULT_RT_START, Enclave, EnclaveId, EnclaveIdGenerator, EnclaveIdx, EnclaveType, Error,
    LinuxServiceEnclaveList, LinuxUserEnclave, link_remain_frame,
};
use crate::{lse::LSEManager, lue::LinuxUserEnclaveList};

const EXT_ID: usize = sbi::ecall::SBI_EXT_HTEE_ENCLAVE;
const CREATE_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize;
const DESTROY_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMDestroyEnclave as usize;

pub const RT_VADDR_START: usize = 0xFFFF_FFFF_8000_0000;
pub const BIN_VADDR_START: usize = 0x20_0000_0000;

pub struct EnclaveCreateEcall;

impl<T> Ecall<T> for EnclaveCreateEcall
where
    T: LSEManager,
    T: LueCtl,
    T: Extension<Device>,
    T: Extension<LinuxUserEnclaveList>,
    T: Extension<Hsm>,
{
    const EXT_ID_RANGE: Range<usize> = id_range(EXT_ID, EXT_ID);
    const FUNC_ID_RANGE: Range<usize> = id_range(CREATE_ENC, DESTROY_ENC);

    fn call(
        &self,
        sm: &T,
        regs: &mut TrapRegs,
        func_id: usize,
        _: usize,
    ) -> Result<EcallResult, extension::EcallError> {
        const USER_ENC: usize = EnclaveType::User as usize;
        const SER_ENC: usize = EnclaveType::Service as usize;

        // struct SharedAllocator<'a>(RefCell<&'a mut BuilderAllocator>);

        // impl<'a> FrameAllocator for SharedAllocator<'a> {
        //     fn alloc(&self) -> Option<PhysPageNum> {
        //         let mut allocator = self.0.borrow_mut();
        //         allocator.alloc_frame()
        //     }

        //     fn dealloc(&self, _: vm::PhysPageNum) {
        //         unimplemented!()
        //     }
        // }
        log::debug!("mepc: {:#x}", regs.mepc);

        match func_id {
            CREATE_ENC => match regs.a1 {
                // linux service enclave
                USER_ENC => {
                    unsafe { riscv::register::mcountinhibit::clear_cy() };
                    let cycle_start = riscv::register::cycle::read();

                    log::info!("create Linux User Enclave");
                    let satp = satp::read();
                    debug_assert_ne!(regs.a0, 0);
                    let load_info = unsafe {
                        let paddr = VirtAddr(regs.a0)
                            .translate(satp.ppn(), satp.mode(), &BarePtReader)
                            .unwrap();

                        &*(paddr.0 as *const LueInfo)
                    };
                    let (lue, args) = sm
                        .create_lue(
                            load_info,
                            RT_VADDR_START as *const u8,
                            BIN_VADDR_START as *const u8,
                            // allocator,
                        )
                        .map_err(|e| {
                            log::error!("create lue failed: {}", e);
                            extension::EcallError::code(0x1)
                        })?;

                    log::debug!("boot args: {args}");

                    let retval = lue.id.0;
                    sm.update(|list: &mut LinuxUserEnclaveList| list.push(lue));

                    let cycle_finish = riscv::register::cycle::read();
                    let cycles = cycle_finish - cycle_start;
                    log::info!("cycle/page: {:#x}", cycles / load_info.mem.page_num);
                    unsafe { riscv::register::mcountinhibit::set_cy() };

                    Ok(EcallResult::ret().retval(retval))
                }
                SER_ENC => {
                    log::debug!("creating linux service enclave");
                    log::debug!("{:#x}, {:#x}", sepc::read(), scause::read().bits());
                    sm.create_lse(regs.a0)
                        .map(|enc| EcallResult::ret().retval(enc.id.0 as usize))
                        .map_err(|e| {
                            log::error!("create lse failed: {}", e);
                            extension::EcallError::code(0x1)
                        })
                }
                _ => {
                    log::error!("unsupported enclave type");
                    log::debug!("{:#x}, {:#x}", sepc::read(), scause::read().bits());
                    Err(extension::EcallError::code(0x1))
                }
            },
            DESTROY_ENC => Err(extension::EcallError::code(0x1)),
            _ => Err(extension::EcallError::code(0x1)),
        }
    }
}

const LAUNCH_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMRunEnclave as usize;
const RESUME_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMResumeEnclave as usize;
const PAUSE_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMStopEnclave as usize;
const EXIT_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMExitEnclave as usize;

use crate::lue::LueCtl;

pub struct EnclaveCtlEcall;

impl<T> Ecall<T> for EnclaveCtlEcall
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
    const EXT_ID_RANGE: Range<usize> = id_range(EXT_ID, EXT_ID);
    const FUNC_ID_RANGE: Range<usize> = id_range(LAUNCH_ENC, EXIT_ENC);

    fn call(
        &self,
        sm: &T,
        regs: &mut TrapRegs,
        func_id: usize,
        _: usize,
    ) -> Result<EcallResult, extension::EcallError> {
        match func_id {
            LAUNCH_ENC => {
                let eid = EnclaveId::from(regs.a0);
                log::debug!("Launch enclave. Id: #{}", eid);
                let sregs = SupervisorRegs::dump();
                if let Some(enc) = sm.view(|list: &LinuxUserEnclaveList| list.get(eid)) {
                    enc.nw_ctx.sregs = sregs;
                    enc.nw_ctx.tregs = regs.clone();
                    // set mepc to the next instruction
                    enc.nw_ctx.tregs.mepc += 0x2;

                    enc.pmp_record.start();

                    let err = sm.launch_lue(DEFAULT_RT_START, enc).err().unwrap();
                    log::error!("launch lue failed: {}", err);
                    Err(extension::EcallError::code(err as usize))
                } else {
                    log::error!("lue #{} not found", eid);
                    Err(extension::EcallError::code(
                        Error::InvalidEnclaveId as usize,
                    ))
                }
            }
            RESUME_ENC => {
                unsafe { riscv::register::mcountinhibit::clear_cy() };
                let cycle_start = riscv::register::cycle::read();

                let eid = EnclaveId::from(regs.a0);
                // unimp length
                regs.mepc += 0x2;

                log::debug!("hart {} resuming enclave #{eid}", mhartid::read());
                let enc = sm
                    .view(|lue: &LinuxUserEnclaveList| lue.get(eid).ok_or(Error::InvalidEnclaveId))
                    .map_err(|e| {
                        log::error!("{e}");
                        extension::EcallError::code(e as usize)
                    })?;

                enc.data.switch_cycle.begin();

                // set current enclave
                log::debug!("Set current enclave to #{eid}, idx: {}", enc.idx());
                sm.view(|hsm: &Hsm| {
                    hsm.current().clean_pmp();
                    hsm.current().set_priv(enc.idx())
                });

                // restore the pmp status
                enc.data.pmp_cache.restore();
                log::debug!("restore pmp entires");

                // save new context
                enc.nw_ctx.save(&regs);
                // enc.data.host_ctx.save(&regs);
                // enclave.save_host_ctx(&tregs);
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
            PAUSE_ENC => {
                unsafe { riscv::register::mcountinhibit::clear_cy() };
                let cycle_start = riscv::register::cycle::read();

                log::debug!("hart {} pausing enclave", mhartid::read());
                // SAFETY: It is safe to convert EnclaveIdx to Enclave<()>
                let enc = sm
                    .view(|hsm: &Hsm| hsm.current().get_priv::<EnclaveIdx>())
                    .map(|idx| {
                        log::debug!("current enclave idx: {}", idx);
                        idx
                    })
                    .unwrap()
                    .as_enc();

                match enc.get_type() {
                    EnclaveType::User => {
                        let enc = enc.as_enc::<LinuxUser>().unwrap();
                        sm.pause_lue(enc, regs)
                            .map(|_| {
                                log::debug!("pause enclave, return to {:#x}", regs.mepc);
                                use pmp::NwCacheExt;
                                sm.apply_nw_pmp_cache();
                                // let cycle_finish = riscv::register::cycle::read();
                                // log::info!(
                                //     "cycle in pausing enclave: {:#x}",
                                //     cycle_finish - cycle_start
                                // );
                                // unsafe { riscv::register::mcountinhibit::set_cy() };
                                EcallResult::ret().retval(regs.a1).fixed_epc()
                            })
                            .map_err(|e| {
                                log::error!("pause enclave failed: {}", e);
                                extension::EcallError::code(e as usize)
                            })
                    }
                    EnclaveType::Service => {
                        log::error!("service enclave cannot be paused");
                        Err(extension::EcallError::code(0x1))
                    }
                    _ => {
                        log::error!("unsupported enclave type");
                        Err(extension::EcallError::code(0x1))
                    }
                }
                // let eid = sm.pause_lue(, regs),
            }
            EXIT_ENC => {
                unsafe { riscv::register::mcountinhibit::clear_cy() };
                let cycle_start = riscv::register::cycle::read();

                // SAFETY: Enclave<()> is safe
                let enc = sm
                    .view(|hsm: &Hsm| hsm.current().get_priv::<EnclaveIdx>())
                    .unwrap()
                    .as_enc();
                let owner = enc.id;

                enc.print_records();

                if let Some(enc) = enc.as_enc::<LinuxUser>() {
                    log::info!("enclave pause num: {}", enc.data.pause_num);
                }

                log::info!("[SM] Cleaning enclave {}", owner);

                sm.update(|list: &mut LinuxUserEnclaveList| list.remove(owner));
                // enc.normal_ctx.sregs.write_satp();
                let satp = enc.nw_vma.satp;
                *regs = unsafe { enc.nw_ctx.restore() };
                // regs.a0 = EnclaveIdx::HOST.into();

                // clean memory content
                // SAFETY: it is safe to clean the enclave memory content by using host satp.
                // 因为，如果操作系统去掉了某个页的映射，那SM就不会复原这个页的所有者，这会导致这个页永远也无法被访问。
                let page_num = align_up!(enc.nw_vma.size, PAGE_SIZE) / PAGE_SIZE;
                let vaddr = enc.nw_vma.start;
                for i in 0..page_num {
                    let vaddr = VirtAddr::from(vaddr + i * PAGE_SIZE);
                    let paddr = vaddr
                        .translate(satp.ppn(), satp.mode(), &BarePtReader)
                        .unwrap();
                    // let pma = sm.pma_mgr.read().get_pma(paddr).unwrap();
                    let pma = sm.view(|mgr: &PhysMemAreaMgr| mgr.get_pma(paddr)).unwrap();
                    let pma_owner = pma.get_prop().get_owner();
                    // we still need to check the owner of the page, avoiding cleaning the page that is not owned by the enclave
                    if pma_owner == owner {
                        unsafe { clean_page_content(paddr.0 as usize) };
                        sm.update(|mgr: &mut PhysMemAreaMgr| {
                            mgr.insert_page(
                                paddr,
                                PmaProp::empty()
                                    .owner(EnclaveId::HOST.0)
                                    .permission(Permission::RWX),
                            );
                        });
                    } else if pma_owner == EnclaveId::EVERYONE {
                        sm.update(|mgr: &mut PhysMemAreaMgr| {
                            mgr.insert_page(
                                paddr,
                                PmaProp::empty()
                                    .owner(EnclaveId::HOST.0)
                                    .permission(Permission::RWX),
                            );
                        });
                    } else {
                        log::error!("cleaning pma {pma} owned by {}", pma_owner);
                        panic!(
                            "[SM] Invalid pma owner in cleaning enclave. The correct owner should be {owner} or {}, but got {}.",
                            EnclaveId::EVERYONE,
                            pma_owner
                        );
                    }
                }

                // enc.host_ctx.sregs.write_satp();
                sm.view(|hsm: &Hsm| hsm.current().clear_priv());
                log::info!("[SM] Enclave {} cleaned", owner);

                let cycle_finish = riscv::register::cycle::read();
                let cycles = cycle_finish - cycle_start;
                log::info!("cycle/page in clean: {:#x}", cycles / page_num);
                unsafe { riscv::register::mcountinhibit::set_cy() };

                Ok(EcallResult::ret().retval(0).fixed_epc())
            }
            _ => Err(extension::EcallError::UnsupportedFunc),
        }
    }
}

/// The caller should never use the content again
unsafe fn clean_page_content(page: usize) {
    let addr = page as *mut u8;
    for i in 0..4096 {
        unsafe { addr.add(i).write(0) };
    }
}
