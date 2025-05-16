pub mod resume_enclave {
    use htee_console::log;
    use riscv::register::mstatus;
    use sbi::TrapRegs;

    use crate::{Error, hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMResumeEnclave as usize;

    /// Resume enclave.
    pub fn call(tregs: &mut TrapRegs) -> Result<(), Error> {
        let eid = tregs.a0;
        log::debug!("Resuming enclave #{eid}");
        let hs = hart::current();
        let enclave = sm().search_lue(eid).ok_or(Error::InvalidEnclaveId(eid))?;
        // set current enclave
        log::debug!("Set current enclave to #{eid}, idx: {}", enclave.idx());
        hs.set_enc(enclave.idx());
        // unimp length
        tregs.mepc += 0x2;
        log::debug!("Save host context");
        enclave.save_host_ctx(&tregs);
        log::debug!("Restore enclave #{eid} context");
        *tregs = unsafe { enclave.restore_enclave_ctx() };
        unsafe { hart::current().switch_mode(tregs.mepc, mstatus::MPP::Supervisor) };
        Ok(())
    }
}

pub mod launch_enclave {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{Error, SecMonitor};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMRunEnclave as usize;

    pub fn call(sm: &SecMonitor, id: usize, tregs: &mut TrapRegs) -> Result<!, Error> {
        log::debug!("Launch enclave. Id: {:#x}", id);
        let e = sm.launch_lue(tregs).err().unwrap();
        tregs.a0 = 0;
        Err(e)
    }
}

pub mod create_enclave {
    use enclave::{EnclaveId, EnclaveType};
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{Error, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize;

    pub fn call(regs: &mut TrapRegs) -> Result<EnclaveId, Error> {
        let sm = sm();
        let ety = regs.a1;
        let a0 = regs.a0;
        log::debug!("Create new enclave");
        let id = if ety == EnclaveType::Driver as usize {
            log::info!("Create linux driver enclave");
            let id = sm.create_lde(a0)?;
            log::info!("Linux driver enclave created: {}", id);
            regs.a0 = id.into();
            let err = sm.launch_lde(regs).err().unwrap();
            log::error!("[SM] Launch linux driver enclave error: {err}");
            return Err(err);
        } else if ety == EnclaveType::User as usize {
            log::info!("Create linux user enclave");
            sm.create_user_enclave(a0)?
        } else {
            return Err(Error::InvalidEnclaveType);
        };
        Ok(id)
    }
}

pub mod enclave_ctl {
    use htee_channel::op::{Head, OpCode};
    use htee_console::log;
    use riscv::register::Permission;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEnclaveCtl as usize;

    pub fn call(regs: &mut TrapRegs) {
        let sm = sm();
        log::debug!("[SM] Ecall enclave control");
        let head = Head::from_ptr(regs.a0 as *const Head);

        // check the owner of the head page
        let pma = sm.pma_mgr.read().get_pma(regs.a0).unwrap();
        if pma.get_prop().get_owner() != hart::current().get_enc_ptr().unwrap().get_id() {
            regs.mepc += 0x2;
            regs.a0 = usize::MAX;
            log::error!("[SM] Ecall enclave control invalid head");
            return;
        }

        let op = head.op_code;
        match op {
            OpCode::READ => {
                let enclave = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
                if let Err(e) = sm.update_ctl_head(enclave, head, Permission::RW, regs) {
                    regs.mepc += 0x2;
                    regs.a0 = usize::MAX;
                    log::error!("[SM] Ecall enclave control read error: {e}");
                }
                unsafe { hart::current().redirect_ecall(enclave.idx(), regs, regs.mepc) };
            }
            OpCode::WRITE => {
                let enclave = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
                if let Err(e) = sm.update_ctl_head(enclave, head, Permission::RW, regs) {
                    regs.mepc += 0x2;
                    regs.a0 = usize::MAX;
                    log::error!("[SM] Ecall enclave control write error: {e}");
                }
                unsafe { hart::current().redirect_ecall(enclave.idx(), regs, regs.mepc) };
            }
            OpCode::FINISH_READ | OpCode::FINISH_WRITE => {
                // FINISH_READ contains two args, arg0 is the head address, arg1 is the status
                let enclave = hart::current().get_enc_ptr().unwrap().as_lue().unwrap();
                match sm.ecall_finish_ctl(enclave, head, regs.a1, regs) {
                    Ok(tregs) => {
                        // update the current hart back to the context of the caller
                        *regs = tregs;
                        regs.mepc += 0x2;
                    }
                    Err(e) => {
                        log::error!("[SM] Ecall enclave control finish control error: {e}");
                        regs.mepc += 0x2;
                        regs.a0 = usize::MAX;
                    }
                }
            }
            _ => {
                regs.mepc += 0x4;
                log::error!("[SM] Invalid op code");
            }
        }
    }
}

pub mod enter_lde {
    use htee_console::log;
    use riscv::register::{mhartid, satp, scause, sscratch, sstatus};
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEneterLde as usize;

    pub fn call(tregs: &mut TrapRegs) {
        log::debug!("Enter lde");
        if let Err(e) = sm().enter_lde(tregs) {
            log::error!("[SM] Enter lde error: {e}");
            unsafe { hart::current().return_to_smode(tregs, false) };
            return;
        }
        let enclave = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
        // tregs.a0 = mhartid::read();
        // tregs.a1 = satp::read().bits();

        unsafe {
            hart::current().redirect_ecall(enclave.idx(), tregs, tregs.mepc + 0x2);
            scause::write(scause::Exception::UserEnvCall as usize);
            sstatus::set_spp(sstatus::SPP::User);
        };
        // scause::set(Tra);
        // tregs.a0 = tregs.sp;
        // we need to setup the sp, as the spp in stvec is set to supervisor
        // tregs.sp = sscratch::read();
    }
}

pub mod exit_lde {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMExitLde as usize;

    pub fn call(tregs: &mut TrapRegs) {
        log::debug!("Exit lde");
        if let Err(e) = sm().exit_lde(tregs) {
            log::error!("[SM] Exit lde error: {e}");
        }
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

/// ELock instruction extension is designed for the enclave to lock the memory region.
/// The memory area including the mmio area and normal physical memory area.
///
/// Memory area that locked, will be unaccessed by anyone except the owner.
///
/// The caller must be in the Linux Driver Enclave(LDE). This is achieved by checking
/// the current hart state.
///
/// Firstly, the sm will check the ownership of the request memory physical area.
/// The memory physical area must belong to the host.
///
/// Secondly, the sm will check if the memory physical area is already locked.
/// If it is, the sm will return an error.
///
/// Thirdly, the sm will change the ownership of the memory physical area to current enclave.
pub mod elock {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMELock as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let vaddr = tregs.a0;
        let size = tregs.a1;
        log::debug!("ELock {:#x}..{:#x}", vaddr, vaddr + size);
        if let Err(e) = sm().lock_mem(hart::current().enclave, vaddr, size) {
            log::error!("[SM] ELock error: {e}");
        }
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

/// EFree instruction extension is designed for the enclave to unlock the memory region
/// which is locked by ELock.
///
/// Note: since the memory region is belonged to host before it locked, efree
/// will give it back to host.
pub mod efree {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEFree as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let vaddr = tregs.a0;
        let size = tregs.a1;
        log::debug!("EFree {:#x}..{:#x}", vaddr, vaddr + size);
        if let Err(e) = sm().free_mem(hart::current().enclave, vaddr, size) {
            log::error!("[SM] EFree error: {e}");
        }
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

pub mod channel_open {
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMChannelOpen as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let arg0 = tregs.a0;
        let arg1 = tregs.a1;
        let eid = hart::current().get_enc_ptr().unwrap().get_id();
        let cid = sm().alloc_channel(eid, arg0, arg1).unwrap();
        tregs.a0 = cid;
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

pub mod channel_close {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMChannelClose as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let cid = tregs.a0;
        if let Err(e) = sm().dealloc_channel(cid) {
            log::error!("[SM] Channel close error: {e}");
        }
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

pub mod channel_connect {
    use htee_console::log;
    use sbi::TrapRegs;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMChannelConnect as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let cid = tregs.a0;
        let eid = hart::current().get_enc_ptr().unwrap().get_id();
        match sm().connect_channel(cid, eid) {
            Ok(arg0) => {
                tregs.a0 = arg0 as usize;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
            Err(e) => {
                log::error!("[SM] Channel connect error: {e}");
                tregs.a0 = usize::MAX;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
        }
    }
}

pub mod copy_from_lue {
    use enclave::EnclaveId;
    use htee_console::log;
    use riscv::register::satp;
    use sbi::TrapRegs;
    use vm::VirtAddr;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCopyFromLue as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let eid = hart::current().get_enc_ptr().unwrap().get_id();

        match sm().attach_channel(eid, |c| {
            let src = (
                VirtAddr::from(tregs.a1),
                context::satp_from_bits(c.arg1 as usize),
                EnclaveId::from(c.lue.unwrap()),
            );
            let dst = (
                VirtAddr::from(tregs.a0),
                satp::read(),
                EnclaveId::from(c.lde.unwrap()),
            );
            sm().copy_data(src, dst, tregs.a2)
        }) {
            Ok(_) => {
                tregs.a0 = 0;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
            Err(e) => {
                log::error!("[SM] Copy from lue error: {e}");
                tregs.a0 = usize::MAX;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
        }
    }
}

pub mod copy_to_lue {
    use enclave::EnclaveId;
    use htee_console::log;
    use riscv::register::satp;
    use sbi::TrapRegs;
    use vm::VirtAddr;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCopyToLue as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let eid = hart::current().get_enc_ptr().unwrap().get_id();

        match sm().attach_channel(eid, |c| {
            let src = (
                VirtAddr::from(tregs.a1),
                satp::read(),
                EnclaveId::from(c.lde.unwrap()),
            );
            let dst = (
                VirtAddr::from(tregs.a0),
                context::satp_from_bits(c.arg1 as usize),
                EnclaveId::from(c.lue.unwrap()),
            );
            sm().copy_data(src, dst, tregs.a2)
        }) {
            Ok(_) => {
                tregs.a0 = 0;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
            Err(e) => {
                log::error!("[SM] Copy from lue error: {e}");
                tregs.a0 = usize::MAX;
                unsafe { hart::current().return_to_smode(tregs, false) };
            }
        }
    }
}

pub mod copy_from_kernel {
    use enclave::EnclaveId;
    use htee_console::log;
    use riscv::register::satp;
    use sbi::TrapRegs;
    use vm::VirtAddr;

    use crate::{hart, sm};

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCopyFromKernel as usize;

    pub fn call(tregs: &mut TrapRegs) {
        let lde = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
        let host_satp = lde.host_ctx.sregs.satp;
        let src = (VirtAddr::from(tregs.a1), host_satp, EnclaveId::HOST);
        let dst = (
            VirtAddr::from(tregs.a0),
            satp::read(),
            EnclaveId::from(lde.id),
        );
        if let Err(e) = sm().copy_data(src, dst, tregs.a2) {
            log::error!("[SM] Copy to kernel error: {e}");
            tregs.a0 = usize::MAX;
        } else {
            tregs.a0 = 0;
        }
        unsafe { hart::current().return_to_smode(tregs, false) };
    }
}

// pub mod copy_to_kernel {
//     use enclave::EnclaveId;
//     use htee_console::log;
//     use riscv::register::satp;
//     use sbi::TrapRegs;
//     use vm::VirtAddr;

//     use crate::{hart, sm};

//     pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCopyToKernel as usize;

//     pub fn call(tregs: &mut TrapRegs) {
//         let lde = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
//         let host_satp = lde.host_ctx.sregs.satp;
//         let src = (
//             VirtAddr::from(tregs.a1),
//             satp::read(),
//             EnclaveId::from(lde.id),
//         );
//         let dst = (VirtAddr::from(tregs.a0), host_satp, EnclaveId::HOST);
//         if let Err(e) = sm().copy_data(src, dst, tregs.a2) {
//             log::error!("[SM] Copy to kernel error: {e}");
//             tregs.a0 = usize::MAX;
//         } else {
//             tregs.a0 = 0;
//         }
//         unsafe { hart::current().return_to_smode(tregs, false) };
//     }
// }
