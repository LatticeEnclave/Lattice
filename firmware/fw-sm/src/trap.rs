pub static mut SBI_HANDLER: usize = 0;

pub(crate) mod tmp {
    use htee_console::log;
    use riscv::register::{mcause, mtval};
    use sm::{trap::ProxyResult, TrapEntry, TrapProxy};

    use super::SBI_HANDLER;

    struct Handler;

    impl TrapProxy for Handler {
        fn handle(regs: &mut sbi::TrapRegs) -> ProxyResult {
            log::debug!("mepc: {:#x}", regs.mepc);
            log::debug!("mtval: {:#x}", mtval::read());
            log::debug!("mcause: {:#x}", mcause::read().bits());

            panic!()
        }
    }

    #[repr(align(0x10))]
    pub fn trap_handler() -> ! {
        Handler::proxy()
    }
}

pub(crate) mod direct {
    use htee_console::log;
    use riscv::register::mcause;
    use sm::{trap::ProxyResult, TrapEntry, TrapProxy};

    use super::SBI_HANDLER;

    struct Handler;

    impl TrapProxy for Handler {
        fn handle(regs: &mut sbi::TrapRegs) -> ProxyResult {
            let trap = mcause::read().cause();
            let res = match trap {
                mcause::Trap::Exception(e) => sm::handle_exception(e, regs),
                mcause::Trap::Interrupt(i) => sm::handle_interrupt(i, regs),
            };
            let cf = res.unwrap_or_else(|e| {
                log::error!("{e}");
                ProxyResult::Continue
            });

            cf
        }
    }

    #[inline(always)]
    pub unsafe fn init_handler(sbi_handler: usize) {
        SBI_HANDLER = sbi_handler;

        let new_inst = TrapEntry::new()
            .target(sbi_handler)
            .to_jal(Handler::_redirect_sbi as usize)
            .unwrap();

        *(Handler::_redirect_sbi as *mut fn() as *mut u32) = new_inst;
    }

    #[repr(align(0x10))]
    pub fn trap_handler() -> ! {
        Handler::proxy()
    }
}

// pub(crate) mod exception {
//     use htee_console::log;
//     use riscv::register::{mcause, mhartid};
//     use sbi::ecall::{enclave_sbi_call_from_usize, SBISMEnclaveCall, SBI_EXT_HTEE_ENCLAVE};
//     use sm::{trap::ProxyResult, TrapProxy, SM};

//     pub struct Proxy;

//     impl TrapProxy for Proxy {
//         fn handle(regs: &mut sbi::TrapRegs) -> ProxyResult {
//             let cause = mcause::read();

//             let exception = cause
//                 .is_exception()
//                 .then(|| mcause::Exception::from(cause.code()))
//                 .unwrap();

//             match exception {
//                 mcause::Exception::IllegalInstruction => handle_illegalinstruction(regs),
//                 mcause::Exception::LoadFault
//                 | mcause::Exception::StoreFault
//                 | mcause::Exception::InstructionFault => {
//                     unsafe {
//                         SM.assume_init_ref().handle_store_fault(regs).unwrap();
//                     }
//                     ProxyResult::Return
//                 }
//                 // handle the ecall from S-mode ecall
//                 mcause::Exception::SupervisorEnvCall => {
//                     if regs.a7 == SBI_EXT_HTEE_ENCLAVE {
//                         return unsafe { SM.assume_init_ref().handle_ecall(regs) };
//                     } else {
//                         return ProxyResult::Continue;
//                     }
//                 }
//                 mcause::Exception::Breakpoint => {
//                     log::debug!("hit ebreak");
//                     return ProxyResult::Continue;
//                 }
//                 _ => return ProxyResult::Continue,
//             }
//         }
//     }

//     fn handle_enclave_ecall(regs: &mut sbi::TrapRegs) -> ProxyResult {
//         let n = enclave_sbi_call_from_usize(regs.a6);
//         match n {
//             Some(fid) => match fid {
//                 SBISMEnclaveCall::SbiSMStopEnclave => {
//                     clear_genera_regs();
//                     ProxyResult::Continue
//                 }
//                 _ => ProxyResult::Return,
//             },
//             None => ProxyResult::Return,
//         }
//     }

//     fn clear_genera_regs() {
//         unsafe { sbi::clear_genera_regs!() }
//     }

//     fn handle_illegalinstruction(regs: &mut sbi::TrapRegs) -> ProxyResult {
//         log::debug!(
//             "IllegalInstruction in hart {} at mepc: {:#x}",
//             mhartid::read(),
//             regs.mepc
//         );

//         let res = unsafe { SM.assume_init_ref().handle_illegal_inst(regs) };
//         match res {
//             Ok(res) => res,
//             Err(e) => {
//                 log::error!("Error: {:?}", e);
//                 ProxyResult::Return
//             }
//         }
//     }
// }

// pub(crate) mod msoft {
//     use sm::{trap::ProxyResult, TrapProxy, SM};

//     pub struct Proxy;

//     impl TrapProxy for Proxy {
//         fn handle(regs: &mut sbi::TrapRegs) -> ProxyResult {
//             unsafe { SM.assume_init_ref().handle_msoft_trap(regs) }
//         }
//     }
// }
