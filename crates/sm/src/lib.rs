#![no_std]
#![feature(naked_functions)]
#![feature(never_type)]

use core::{mem::MaybeUninit, panic};

use clint::ClintClient;
use context::sstatus_read_bit;
pub use device::DeviceInfo;
use enclave::{Enclave, EnclaveId, EnclaveIdx};
pub use error::Error;
use extension::{Ecall, EcallError, Extension};
use heapless::Vec;
use hsm::Hsm;
use htee_console::log;
pub use pma::{Owner, PhysMemArea, PhysMemAreaMgr, PmaProp};
use pmp::{MAX_PMP_COUNT, NwCacheExt, PmpHelper};
pub use pmp::{PMP_COUNT, PmpStatus};
use riscv::register::{mcause, mhartid, mscratch, mtval, satp, scause, sepc, stvec};
use sbi::TrapRegs;
pub use sm::SecMonitor;
use trap::ProxyResult;
pub use trap::{TrapEntry, TrapProxy, TrapVec};

// mod clint;
pub mod consts;
mod device;
mod dma;
// mod ecall;
mod error;
// mod hart;
mod helper;
// mod inst_ext;
// mod pma;
// mod pmp;
mod heap;
mod pt;
mod sm;
pub mod trap;

pub static mut SM: MaybeUninit<SecMonitor> = MaybeUninit::uninit();

#[inline(always)]
pub fn sm() -> &'static sm::SecMonitor {
    #[allow(static_mut_refs)]
    unsafe {
        SM.assume_init_ref()
    }
}

#[inline(always)]
pub fn handle_interrupt(
    interrupt: mcause::Interrupt,
    regs: &mut TrapRegs,
) -> Result<ProxyResult, Error> {
    let res = match interrupt {
        mcause::Interrupt::MachineSoft => handle_msoft_trap(regs),
        _ => ProxyResult::Continue,
    };

    Ok(res)
}

pub fn handle_msoft_trap(_: &mut TrapRegs) -> ProxyResult {
    let sm = sm();
    sm.view(|hsm: &Hsm| {
        let op = hsm.take_op();
        if op.clean_pmp {
            if hsm.current().get_priv::<EnclaveIdx>().is_none() {
                // in normal world
                hsm.current().clean_pmp();
                // log::debug!("hart #{} cleand pmp", mhartid::read());
            }
            sm.view(|clint: &ClintClient| clint.reset_msip());
        }
    });
    ProxyResult::Continue
}

pub fn handle_exception(
    exception: mcause::Exception,
    regs: &mut TrapRegs,
) -> Result<ProxyResult, Error> {
    use sbi::ecall::SBI_EXT_HTEE_ENCLAVE;

    let res = match exception {
        mcause::Exception::IllegalInstruction => {
            if regs.a7 == SBI_EXT_HTEE_ENCLAVE && mtval::read() == 0 {
                // unimp width
                handle_ecall(regs, 0x2)
            } else {
                ProxyResult::Continue
            }
        }
        mcause::Exception::LoadFault
        | mcause::Exception::StoreFault
        | mcause::Exception::InstructionFault => match handle_pmp_fault(regs) {
            Ok(_) => ProxyResult::Return,
            Err(e) => {
                log::trace!("[SM] {e}");
                // unsafe { regs.redirect_to_smode() };
                ProxyResult::Continue
            }
        },
        mcause::Exception::Breakpoint => {
            log::warn!(
                "hart {} get breakpiont:
mepc: {:#x}, mscratch: {:#x}
satp: {:#x}, stvec:{:#x}
ra:{:#x}, sp:{:#x}",
                mhartid::read(),
                regs.mepc,
                mscratch::read(),
                satp::read().bits(),
                stvec::read().bits(),
                regs.ra,
                regs.sp,
            );
            ProxyResult::Continue
        }
        // handle the ecall from S-mode ecall
        mcause::Exception::SupervisorEnvCall => {
            if regs.a7 == SBI_EXT_HTEE_ENCLAVE {
                // ecall instruction length
                handle_ecall(regs, 0x4)
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

// pub fn handle_illegal_inst(regs: &mut TrapRegs) -> Result<ProxyResult, Error> {
//     use sbi::ecall::SBI_EXT_HTEE_ENCLAVE;

//     if regs.a7 != SBI_EXT_HTEE_ENCLAVE || mtval::read() != 0x0 {
//         return Ok(ProxyResult::Continue);
//     }
//     log::debug!(
//         "hart #{} handle illegal instruction at {:#x}",
//         mhartid::read(),
//         regs.mepc
//     );
//     handle_inst_ext(regs)
// }

pub fn handle_ecall(regs: &mut TrapRegs, offset: usize) -> ProxyResult {
    use enclave::ecall::*;
    use extension::EcallHandler;

    // let hartid = mhartid::read();
    // log::debug!("hart #{hartid} handle_ecall at {:#x}", regs.mepc);
    // log::debug!("ext id: {}", regs.a7);
    // log::debug!("func id: {}", regs.a6);

    #[cfg(debug_assertions)]
    check_stack_overflow();

    let res = EcallHandler::new()
        .with_handler(EnclaveCreateEcall)
        .with_handler(EnclaveCtlEcall)
        .call(sm(), regs, regs.a6, regs.a7);

    let res = match res {
        Ok(r) => {
            if !r.fixed_epc {
                unsafe {
                    regs.fix_mepc(offset);
                }
            }
            regs.a0 = 0;
            regs.a1 = r.retval;
            regs.a6 = 0;
            regs.a7 = 0;
            // we will clean a6, a7
            ProxyResult::Return
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
                    regs.a6,
                    regs.a7,
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

    // let handler = EcallHandler:

    // let a6 = regs.a6;
    // log::debug!("ecall id: {a6}");
    // match a6 {
    //     pause_enclave::ID => pause_enclave::call(regs),
    //     clean_enclave::ID => clean_enclave::call(regs),

    //     _ => return ProxyResult::Continue,
    // }

    // ProxyResult::Return
}

#[inline]
pub fn check_stack_overflow() {
    #[inline(always)]
    fn get_sp() -> usize {
        let sp: usize;
        unsafe {
            core::arch::asm!("mv  {}, sp", out(reg) sp);
        }
        sp
    }

    // debug_assert!(get_sp() > mscratch::read() - 0x1000);
    log::debug!("sp: {:#x}, mscratch: {:#x}", get_sp(), mscratch::read());
    if get_sp() < mscratch::read() - 0x1000 {
        log::debug!("sp: {:#x}, mscratch: {:#x}", get_sp(), mscratch::read());
        panic!()
    }
}

#[inline]
pub fn handle_pmp_fault(regs: &mut TrapRegs) -> Result<(), Error> {
    // #[cfg(debug_assertions)]
    check_stack_overflow();

    // let mut mempool = sm().alloc_mempool_spin();
    // let buf = unsafe { mempool.alloc(Vec::new()).unwrap().as_mut() };
    // buf.clear();

    // assert!(buf.is_empty());

    let buf = unsafe { sm().hsm.current().pmp_buf.as_mut() };
    buf.clear();

    handle_all_fault(regs, buf)?;
    // handle_all_fault(regs, &mut Vec::new())?;

    Ok(())
}

#[inline]
fn handle_all_fault(
    regs: &mut TrapRegs,
    buf: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
) -> Result<(), Error> {
    use helper::*;
    use riscv::register::{mepc, mstatus, satp};
    use vm::mm::*;

    unsafe { riscv::register::mcountinhibit::clear_cy() };
    let cycle_start = riscv::register::cycle::read();

    let hartid = mhartid::read();
    log::trace!("hart #{hartid} handle pmp fault at {:#x}", regs.mepc);
    let satp = satp::read();
    let mepc = mepc::read();
    let mtval = mtval::read();
    let mcause = mcause::read().bits();

    let idx = sm().view(|hsm: &Hsm| hsm.current().get_priv::<EnclaveIdx>());
    let enc: Option<&'static mut Enclave<()>> = idx.map(|idx| unsafe { Enclave::from_ptr(idx) });
    let eid = enc
        .map(|enc| {
            enc.pmp_record.start_handle();
            enc.id()
        })
        .unwrap_or(EnclaveId::HOST);
    if let Some(idx) = idx {
        log::trace!("hart {hartid} enclave idx: {idx}");
    } else {
        #[cfg(debug_assertions)]
        {
            let num = sm()
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
        sstatus_read_bit(),
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

    // DMA controller accessing
    // if dma::is_accessing_dma(mtval) {
    //     log::debug!("is accessing dma");
    //     let inst = dma::get_inst_data(mepc);
    //     let (val, size) = dma::get_reg_data(&regs, inst);
    //     // is not enable dma, so we let it write to the register
    //     if !dma::is_dma_enabled(val) {
    //         log::debug!("dma is not enabled, write to the register");
    //         dma::write_dma_regs(val, size);
    //         // unsafe { hart::current().return_to_smode(regs, false) };
    //         unsafe {
    //             regs.enable_interrupt()
    //                 .switch_next_mode(mstatus::MPP::Supervisor as usize)
    //                 .fix_mepc(0x2);
    //         }
    //         return Ok(());
    //     }
    //     log::debug!("dma is enabled, check dma access");
    //     if let Err(e) = dma::check_dma_access() {
    //         log::error!("[SM] {e}");
    //         return Err(e);
    //     }
    //     log::debug!("dma access is valid, write to the register");
    //     dma::write_dma_regs(val, size);
    //     unsafe {
    //         regs.enable_interrupt()
    //             .switch_next_mode(mstatus::MPP::Supervisor as usize)
    //             .fix_mepc(0x2);
    //     }
    //     return Ok(());
    // }

    // switch context may also leads access fault
    // if self.is_context_switch(hartid) {
    //     // we switch the working enclave
    //     // then we need update the pmp registers to continue the execution
    //     log::debug!("Switching enclave");
    //     self.switch_enclave(hartid, regs);
    //     log::debug!("Switched enclave");
    // }

    // sm().view(|hsm: &Hsm| -> Result<(), Error> {
    match satp.mode() {
        satp::Mode::Bare => {
            log::trace!("Bare mode");
            pmas_on_paddr(&sm().pma_mgr.read(), mepc, mtval, buf).unwrap()
        }
        satp::Mode::Sv39 => {
            log::trace!("SV39 mode");
            pmas_req_vaddr(&sm().pma_mgr.read(), mepc, mtval, satp.ppn(), SV39, buf)?
        }
        satp::Mode::Sv48 => {
            log::trace!("SV48 mode");
            pmas_req_vaddr(&sm().pma_mgr.read(), mepc, mtval, satp.ppn(), SV48, buf)?
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
            // return Err(Error::InvalidMemoryAccess { pc: mepc });
        }
    }

    update_pmp_by_pmas(buf, sm().iter_current_pma());

    log::trace!("Updated pmp registers");

    if idx.is_none() {
        // update normal world cache
        sm().update_nw_pmp_cache();
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
