use trap_proxy::{ProxyResult, TrapEntry, TrapProxy};

use crate::sm;

pub struct TrapHandler;

impl TrapProxy for TrapHandler {
    fn handle(regs: &mut sbi::TrapRegs) -> ProxyResult {
        sm().handle_trap(regs)
    }
}

impl TrapHandler {
    #[inline(always)]
    pub unsafe fn init_redirect(sbi_handler: usize) {
        let new_inst = TrapEntry::new()
            .target(sbi_handler)
            .to_jal(TrapHandler::_redirect_sbi as usize)
            .unwrap();

        unsafe { *(TrapHandler::_redirect_sbi as *mut fn() as *mut u32) = new_inst };
    }
}
