/// The adjustment of registers must be done in the call function.
///
pub mod pause_enclave {
    use crate::hart;
    use enclave::EnclaveIdx;
    use htee_console::log;
    use sbi::TrapRegs;

    pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMStopEnclave as usize;

    pub fn call(regs: &mut TrapRegs) {
        log::debug!("Pause enclave");
        let hs = hart::current();
        regs.mepc += 0x4;

        // match hs.get_enc_ptr().unwrap().as_enclave() {
        //     EnclaveRef::User(enclave) => {
        //         // ecall instruction length
        //         let rc = regs.a0;
        //         enclave.enclave_ctx.save(regs);
        //         unsafe {
        //             *regs = enclave.host_ctx.restore();
        //             regs.a0 = enclave.id().into();
        //             regs.a1 = rc;
        //         }
        //     }
        //     EnclaveRef::Driver(enclave) => unsafe {
        //         enclave.enclave_ctx.save(regs);
        //         let rc = regs.a0;
        //         *regs = enclave.host_ctx.restore();
        //         regs.a0 = enclave.id.into();
        //         regs.a1 = rc;
        //     },
        // }
        hs.set_enc(EnclaveIdx::HOST);
    }
}

// pub mod clean_enclave {
//     use crate::{hart, sm};
//     use enclave::{EnclaveId, EnclaveIdx};
//     use htee_console::log;
//     use pma::PmaProp;
//     use riscv::register::{Permission, satp};
//     use sbi::TrapRegs;
//     use vm::{Translate, consts::PAGE_SIZE, page_table::BarePtReader, vm::VirtAddr};

//     pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMExitEnclave as usize;

//     pub fn call(regs: &mut TrapRegs) {
//         let sm = sm();
//         let hs = hart::current();
//         let enclave = hs.get_enc_ptr().unwrap().as_lue().unwrap();
//         let owner = enclave.id;
//         log::info!("[SM] Cleaning enclave {}", owner);
//         enclave.host_ctx.sregs.write_satp();
//         let host_pt = satp::read().ppn();
//         let host_mode = satp::read().mode();

//         // recover host context including general registers and s-mode registers
//         *regs = unsafe { enclave.host_ctx.restore() };
//         regs.a0 = EnclaveIdx::HOST.into();

//         // clean memory content
//         // SAFETY: it is safe to clean the enclave memory content by using host satp.
//         // 因为，如果操作系统去掉了某个页的映射，那SM就不会复原这个页的所有者，这会导致这个页永远也无法被访问。
//         let host_page_num = enclave.host_info.size.div_ceil(PAGE_SIZE);
//         let host_vaddr = enclave.host_info.vaddr;
//         for i in 0..host_page_num {
//             let vaddr = VirtAddr::from(host_vaddr.0 + i * PAGE_SIZE);
//             let paddr = vaddr.translate(host_pt, host_mode, &BarePtReader).unwrap();
//             let pma = sm.pma_mgr.read().get_pma(paddr).unwrap();
//             let pma_owner = pma.get_prop().get_owner();
//             // we still need to check the owner of the page, avoiding cleaning the page that is not owned by the enclave
//             if pma_owner == owner {
//                 unsafe { clean_page_content(paddr.0 as usize) };
//                 sm.pma_mgr.write().insert_page(
//                     paddr,
//                     PmaProp::empty()
//                         .owner(EnclaveId::HOST.0)
//                         .permission(Permission::RWX),
//                 );
//             } else if pma_owner == EnclaveId::EVERYONE {
//                 sm.pma_mgr.write().insert_page(
//                     paddr,
//                     PmaProp::empty()
//                         .owner(EnclaveId::HOST.0)
//                         .permission(Permission::RWX),
//                 );
//             } else {
//                 panic!(
//                     "[SM] Invalid pma owner in cleaning enclave. The correct owner should be {owner} or {}, but got {}.",
//                     EnclaveId::EVERYONE,
//                     pma_owner
//                 );
//             }
//         }

//         hart::current().set_enc(EnclaveIdx::HOST);
//         log::info!("[SM] Enclave {} cleaned", owner);
//     }

//     unsafe fn clean_page_content(page: usize) {
//         let addr = page as *mut u8;
//         for i in 0..4096 {
//             unsafe { addr.add(i).write(0) };
//         }
//     }
// }

// pub mod enclave_ctl {
//     use htee_channel::op::{Head, OpCode};
//     use htee_console::log;
//     use riscv::register::Permission;
//     use sbi::TrapRegs;

//     use crate::{hart, sm};

//     pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEnclaveCtl as usize;

//     pub fn call(regs: &mut TrapRegs) {
//         let sm = sm();
//         log::debug!("[SM] Ecall enclave control");
//         let head = Head::from_ptr(regs.a0 as *const Head);

//         // check the owner of the head page
//         let pma = sm.pma_mgr.read().get_pma(regs.a0).unwrap();
//         if pma.get_prop().get_owner() != hart::current().get_enc_ptr().unwrap().get_id() {
//             regs.mepc += 0x4;
//             regs.a0 = usize::MAX;
//             log::error!("[SM] Ecall enclave control invalid head");
//             return;
//         }

//         let op = head.op_code;
//         match op {
//             OpCode::READ => {
//                 let enclave = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
//                 if let Err(e) = sm.update_ctl_head(enclave, head, Permission::RW, regs) {
//                     regs.mepc += 0x4;
//                     regs.a0 = usize::MAX;
//                     log::error!("[SM] Ecall enclave control read error: {e}");
//                 }
//                 unsafe { hart::current().redirect_ecall(enclave.idx(), regs, regs.mepc) };
//             }
//             OpCode::WRITE => {
//                 let enclave = hart::current().get_enc_ptr().unwrap().as_lde().unwrap();
//                 if let Err(e) = sm.update_ctl_head(enclave, head, Permission::RW, regs) {
//                     regs.mepc += 0x4;
//                     regs.a0 = usize::MAX;
//                     log::error!("[SM] Ecall enclave control write error: {e}");
//                 }
//                 unsafe { hart::current().redirect_ecall(enclave.idx(), regs, regs.mepc) };
//             }
//             OpCode::FINISH_READ | OpCode::FINISH_WRITE => {
//                 // FINISH_READ contains two args, arg0 is the head address, arg1 is the status
//                 let enclave = hart::current().get_enc_ptr().unwrap().as_lue().unwrap();
//                 match sm.ecall_finish_ctl(enclave, head, regs.a1, regs) {
//                     Ok(tregs) => {
//                         // update the current hart back to the context of the caller
//                         *regs = tregs;
//                         regs.mepc += 0x4;
//                     }
//                     Err(e) => {
//                         log::error!("[SM] Ecall enclave control finish control error: {e}");
//                         regs.mepc += 0x4;
//                         regs.a0 = usize::MAX;
//                     }
//                 }
//             }
//             _ => {
//                 regs.mepc += 0x4;
//                 log::error!("[SM] Invalid op code");
//             }
//         }
//     }
// }

// pub mod enter_lde {
//     use htee_console::log;
//     use sbi::TrapRegs;

//     use crate::{hart, sm};

//     pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEneterLde as usize;

//     pub fn call(tregs: &mut TrapRegs) {
//         log::debug!("Enter lde");
//         if let Err(e) = sm().enter_lde(tregs) {
//             log::error!("[SM] Enter lde error: {e}");
//             tregs.mepc += 0x4;
//             return;
//         }
//         let enclave = hart::current().get_enc().unwrap().as_lde().unwrap();
//         unsafe { hart::current().redirect_ecall(enclave.idx(), tregs, tregs.mepc) };
//     }
// }

// pub mod exit_lde {
//     use htee_console::log;
//     use sbi::TrapRegs;

//     use crate::{hart, sm};

//     pub const ID: usize = sbi::ecall::SBISMEnclaveCall::SbiSMEneterLde as usize;

//     pub fn call(tregs: &mut TrapRegs) {
//         log::debug!("Exit lde");
//         let enclave = hart::current().get_enc().unwrap().as_lde().unwrap();
//         if let Err(e) = sm().enter_lde(tregs) {
//             log::error!("[SM] Enter lde error: {e}");
//             tregs.mepc += 0x4;
//             return;
//         }
//         unsafe { hart::current().redirect_ecall(enclave.idx(), tregs, tregs.mepc) };
//     }
// }

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
            tregs.mepc += 0x4;
            return;
        }
        tregs.mepc += 0x4;
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
            tregs.mepc += 0x4;
            return;
        }
        tregs.mepc += 0x4;
    }
}
