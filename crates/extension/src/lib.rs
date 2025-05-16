#![no_std]

use core::{fmt::Display, marker::PhantomData, ops::Range};

use trap_proxy::{ProxyResult, TrapRegs};

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

pub trait Extension<T> {
    fn view<O>(&self, f: impl FnOnce(&T) -> O) -> O;
    fn update<O>(&self, f: impl FnOnce(&mut T) -> O) -> O;
}

pub struct EcallResult {
    pub proxy: ProxyResult,
    pub retval: usize,
    pub fixed_epc: bool,
}

impl EcallResult {
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

pub trait Ecall<T> {
    const EXT_ID_RANGE: Range<usize>;
    const FUNC_ID_RANGE: Range<usize>;

    #[inline(always)]
    fn able(&self, func_id: usize, ext_id: usize) -> bool {
        Self::EXT_ID_RANGE.contains(&ext_id) && Self::FUNC_ID_RANGE.contains(&func_id)
    }

    fn call(
        &self,
        sm: &T,
        regs: &mut TrapRegs,
        func_id: usize,
        ext_id: usize,
    ) -> Result<EcallResult, EcallError>;
}

impl<T> Ecall<T> for () {
    const EXT_ID_RANGE: Range<usize> = 0..0;
    const FUNC_ID_RANGE: Range<usize> = 0..0;

    fn call(&self, _: &T, _: &mut TrapRegs, _: usize, _: usize) -> Result<EcallResult, EcallError> {
        unimplemented!()
    }
}

pub const fn id_range(start: usize, end: usize) -> Range<usize> {
    start..(end + 1)
}

pub struct EcallHandler<T, A: Ecall<T>, B: Ecall<T>> {
    a: A,
    b: B,
    _marker: PhantomData<T>,
}

impl<T, A: Ecall<T>, B: Ecall<T>> Ecall<T> for EcallHandler<T, A, B> {
    const EXT_ID_RANGE: Range<usize> = 0..0;

    const FUNC_ID_RANGE: Range<usize> = 0..0;

    #[inline(always)]
    fn able(&self, func_id: usize, ext_id: usize) -> bool {
        self.a.able(func_id, ext_id) || self.b.able(func_id, ext_id)
    }

    #[inline]
    fn call(
        &self,
        sm: &T,
        regs: &mut TrapRegs,
        func_id: usize,
        ext_id: usize,
    ) -> Result<EcallResult, EcallError> {
        if self.a.able(func_id, ext_id) {
            self.a.call(sm, regs, func_id, ext_id)
        } else if self.b.able(func_id, ext_id) {
            self.b.call(sm, regs, func_id, ext_id)
        } else {
            Err(EcallError::UnsupportedFunc)
        }
    }
}

impl<T, A: Ecall<T>, B: Ecall<T>> EcallHandler<T, A, B> {
    pub const fn with_handler<E: Ecall<T>>(
        self,
        handler: E,
    ) -> EcallHandler<T, E, EcallHandler<T, A, B>> {
        EcallHandler {
            a: handler,
            b: self,
            _marker: PhantomData,
        }
    }
}

impl<T> EcallHandler<T, (), ()> {
    pub const fn new() -> Self {
        Self {
            a: (),
            b: (),
            _marker: PhantomData,
        }
    }
}
