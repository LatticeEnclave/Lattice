use core::fmt::Display;

use sbi::TrapRegs;
use trap_proxy::ProxyResult;

use crate::SecMonitor;

const ERR_CODE_MASK: usize = !(1 << 63);

#[derive(Debug)]
#[repr(usize)]
pub enum EcallError {
    UnsupportedFunc = 0x1 | !ERR_CODE_MASK,
    EcallRuntime(usize),
}

impl EcallError {
    #[inline(always)]
    pub fn code(code: usize) -> Self {
        Self::EcallRuntime(code & ERR_CODE_MASK)
    }
}

impl Display for EcallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedFunc => write!(f, "Unsupported function"),
            Self::EcallRuntime(code) => write!(f, "Error in handling ecall: {}", code),
        }
    }
}

pub struct EcallResult {
    pub proxy: ProxyResult,
    pub retval: usize,
    pub fixed_epc: bool,
}

impl EcallResult {
    #[allow(unused)]
    #[inline(always)]
    pub fn forward() -> Self {
        Self {
            proxy: ProxyResult::Continue,
            retval: 0,
            fixed_epc: false,
        }
    }

    #[inline(always)]
    pub fn ret() -> Self {
        Self {
            proxy: ProxyResult::Return,
            retval: 0,
            fixed_epc: false,
        }
    }

    #[inline(always)]
    pub fn retval(mut self, retval: usize) -> Self {
        self.retval = retval;
        self
    }

    #[inline(always)]
    pub fn fixed_epc(mut self) -> Self {
        self.fixed_epc = true;
        self
    }
}

pub trait HandleEcall {
    fn able(&self, funcid: usize, extid: usize) -> bool;

    fn call(&self, sm: &SecMonitor, regs: &mut TrapRegs) -> Result<EcallResult, EcallError>;
}

impl HandleEcall for () {
    #[inline]
    fn able(&self, _: usize, _: usize) -> bool {
        false
    }

    fn call(&self, _: &SecMonitor, _: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        unimplemented!()
    }
}

pub struct EcallHandler<H, F>
where
    H: HandleEcall,
    F: Fn(&SecMonitor, &mut TrapRegs) -> Result<EcallResult, EcallError>,
{
    extid: usize,
    funcid: usize,
    f: F,
    other: H,
}

impl<H, F> HandleEcall for EcallHandler<H, F>
where
    H: HandleEcall,
    F: Fn(&SecMonitor, &mut TrapRegs) -> Result<EcallResult, EcallError>,
{
    #[inline]
    fn able(&self, func_id: usize, ext_id: usize) -> bool {
        (func_id == self.funcid && ext_id == self.extid) || self.other.able(func_id, ext_id)
    }

    #[inline]
    fn call(&self, sm: &SecMonitor, regs: &mut TrapRegs) -> Result<EcallResult, EcallError> {
        if regs.a6 == self.funcid && regs.a7 == self.extid {
            (self.f)(sm, regs)
        } else if self.other.able(regs.a6, regs.a7) {
            self.other.call(sm, regs)
        } else {
            Err(EcallError::UnsupportedFunc)
        }
    }
}

impl<H, F> EcallHandler<H, F>
where
    H: HandleEcall,
    F: Fn(&SecMonitor, &mut TrapRegs) -> Result<EcallResult, EcallError>,
{
    pub const fn add_ecall<O: Fn(&SecMonitor, &mut TrapRegs) -> Result<EcallResult, EcallError>>(
        self,
        func_id: usize,
        ext_id: usize,
        f: O,
    ) -> EcallHandler<EcallHandler<H, F>, O> {
        EcallHandler {
            funcid: func_id,
            extid: ext_id,
            f,
            other: self,
        }
    }
}

impl<F> EcallHandler<(), F>
where
    F: Fn(&SecMonitor, &mut TrapRegs) -> Result<EcallResult, EcallError>,
{
    pub const fn new(funcid: usize, extid: usize, f: F) -> Self {
        Self {
            funcid,
            extid,
            f,
            other: (),
        }
    }
}