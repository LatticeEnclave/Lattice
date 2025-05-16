use core::slice;

use riscv::register::mtvec;
use sbi::TrapRegs;

pub enum ProxyResult {
    Return = 0,
    Continue = 1,
}

/// A common trap proxier wrapper.
///
/// The trap proxier will mret if `handle` return
pub trait TrapProxy {
    #[inline(always)]
    fn proxy() -> ! {
        unsafe {
            Self::_enter();
            Self::_call_handle();
            Self::_exit()
        }
    }

    fn next_without_handle() -> ! {
        unsafe {
            Self::_redirect_sbi();
        }
    }

    /// Note: typically, user should not directly call this method, as registers will not be saved.
    ///
    /// TrapProxy redirect to SBI if `handle` return true, otherwise mret to mepc.
    fn handle(regs: &mut TrapRegs) -> ProxyResult;

    ///// Initializate the Proxier
    //unsafe fn init(entry: TrapEntry) -> Result<()> {
    //    // overwrite _redirect_sbi func content
    //    let new_inst = entry
    //        .to_jal(Self::_redirect_sbi as usize)
    //        .ok_or(Error::other("jal instruction encoding failed"))?;
    //
    //    unsafe { *(Self::_redirect_sbi as *mut fn() as *mut u32) = new_inst };
    //    Ok(())
    //}

    /// Note: always inline
    #[inline(always)]
    unsafe fn _redirect_sbi() -> ! {
        unsafe {
            core::arch::asm!(
                "j      {0}",
                sym Self::_redirect_sbi,
                options(noreturn)
            )
        }
    }

    #[inline(always)]
    unsafe fn _enter() {
        // save t0, sp, change sp to the sbi stack
        sbi::trap_save_and_setup_sp_t0!();
        sbi::trap_save_mepc_mstatus!();
        sbi::trap_save_general_regs_except_sp_t0!();
    }

    #[inline(always)]
    unsafe fn _jump_next() {
        sbi::trap_restore_a0_t0!();
        unsafe {
            core::arch::asm!(
                "j    {}",
                sym Self::_redirect_sbi,
                options(noreturn)
            )
        }
    }

    #[inline(always)]
    unsafe fn _exit() -> ! {
        sbi::trap_restore_general_regs_except_a0_t0!();
        sbi::trap_restore_mepc_mstatus!();
        unsafe {
            core::arch::asm!(
                "ld     t0, -{}(a0)",
                "bnez   t0, {}",
                const core::mem::size_of::<usize>(),
                sym Self::_jump_next,
            );
            sbi::trap_restore_a0_t0!();
            core::arch::asm!("mret", options(noreturn))
        }
    }

    #[inline(always)]
    unsafe fn _call_handle() {
        unsafe {
            core::arch::asm!(
                "add    a0, sp, zero",
                "call   {}",
                "sd     a0, -{}(sp)",
                "mv     a0, sp",
                sym Self::handle,
                const core::mem::size_of::<usize>()
            )
        }
    }
}

pub struct TrapVec {
    ptr: *const u32,
    mode: mtvec::TrapMode,
}

impl TrapVec {
    /// Read trap vector from current mtvec register
    pub fn from_mtvec() -> Self {
        let addr = mtvec::read().address();
        let mode = mtvec::read().trap_mode().unwrap_or(mtvec::TrapMode::Direct);
        // use address and mode from mtvec
        unsafe { Self::new(addr, mode) }
    }

    /// Create trap vector at @addr with @mode
    ///
    /// Note: if the trap mode is Vectored, the given address must have 48 bytes space at least.
    pub unsafe fn new(addr: impl Into<usize>, mode: mtvec::TrapMode) -> Self {
        let ptr = addr.into() as *mut u32;

        Self { ptr, mode }
    }

    pub fn raw_data(&self) -> &[u32] {
        unsafe { slice::from_raw_parts(self.ptr, 12) }
    }

    pub fn raw_data_mut(&self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u32, 12) }
    }

    #[inline(always)]
    pub fn get_ptr(&self) -> *const u32 {
        self.ptr
    }

    pub fn len(&self) -> usize {
        match self.mode {
            mtvec::TrapMode::Direct => 0,
            mtvec::TrapMode::Vectored => 12,
        }
    }

    /// Clone all entries of another trap vec
    pub fn clone_from_trapvec(&mut self, other: &TrapVec) {
        match (self.mode, other.mode) {
            (mtvec::TrapMode::Direct, mtvec::TrapMode::Direct) => unsafe {
                self.update_entry(0, other.get_exception_entry());
            },
            (mtvec::TrapMode::Vectored, mtvec::TrapMode::Direct) => {
                self.update_all_entry(other.get_exception_entry())
            }
            (mtvec::TrapMode::Vectored, mtvec::TrapMode::Vectored) => unsafe {
                other
                    .raw_data()
                    .iter()
                    .map(|instr| TrapEntry::from_jal(instr))
                    .enumerate()
                    .for_each(|(idx, entry)| {
                        self.update_entry(idx, entry);
                    })
            },
            _ => {}
        }
    }

    /// Update mtvec with self.ptr and self.mode
    pub unsafe fn apply(self) {
        unsafe {
            mtvec::write(self.ptr as usize, self.mode);
        }
    }

    // pub fn read_entry(&self, trap: Trap) -> TrapEntry {
    //     match self.mode {
    //         TrapMode::Vectored => read_entry_vec_mode(self.ptr, trap),
    //         TrapMode::Direct => TrapEntry {
    //             dst: self.ptr as usize,
    //         },
    //     }
    // }

    /// Get first trap entry or
    #[inline(always)]
    pub fn get_exception_entry(&self) -> TrapEntry {
        TrapEntry {
            dst: self.ptr as usize,
        }
    }

    /// Init entire trap vec with entry
    pub fn update_all_entry(&mut self, entry: TrapEntry) {
        let ptr = self.ptr as *mut u32;
        for offset in 0..11 {
            unsafe { update_entry_impl(ptr, offset, entry) }
        }
    }

    pub unsafe fn update_entry(&mut self, idx: usize, entry: TrapEntry) -> &mut Self {
        unsafe { update_entry_impl(self.ptr as *mut u32, idx, entry) };
        self
    }
}

#[derive(Clone, Copy)]
pub struct TrapEntry {
    pub dst: usize,
}

impl TrapEntry {
    pub fn new() -> Self {
        Self { dst: 0 }
    }

    #[inline]
    pub fn target(mut self, dst: usize) -> Self {
        self.dst = dst;
        self
    }

    #[inline]
    pub fn get_target(&self) -> usize {
        self.dst
    }

    pub fn from_jal(instr: &u32) -> Self {
        let pc = instr as *const u32 as usize;
        let val = u32::from_le(*instr);
        let imm = val >> 12;
        let imm_20 = imm >> 19;
        let imm_10_1 = imm >> 9 & 0b1111111111;
        let imm_11 = imm >> 8 & 0b1;
        let imm_19_12 = imm & 0b11111111;

        let offset = imm_10_1 << 1 | imm_11 << 11 | imm_19_12 << 12;
        let target = if imm_20 != 0 {
            pc - offset as usize
        } else {
            pc + offset as usize
        };

        TrapEntry { dst: target }
    }

    pub fn to_jal(self, pc: usize) -> Option<u32> {
        Some(
            Jal {
                src: pc,
                dst: self.dst,
            }
            .to_bytes(),
        )
    }
}

#[derive(Clone, Copy)]
struct Jal {
    pub dst: usize,
    pub src: usize,
}

impl Jal {
    pub fn to_bytes(self) -> u32 {
        create_jal(self.dst, self.src)
    }
}

/// Create the jal instruction at pc which jump to target
#[inline]
fn create_jal(target: usize, pc: usize) -> u32 {
    let offset: i32 = target.overflowing_sub(pc).0 as i32;

    let imm_10_1 = (offset >> 1) & 0b1111111111;
    let imm_11 = (offset >> 11) & 0b1;
    let imm_19_12 = (offset >> 12) & 0b11111111;
    let imm_20 = offset >> 20 & 0b1;

    let imm = (imm_20 << 19 | imm_10_1 << 9 | imm_11 << 8 | imm_19_12) as u32;
    // rd = x0
    let rd = 0b00000;
    // jal opcode
    let opcode = 0b1101111;

    imm << 12 | rd << 7 | opcode
}

unsafe fn update_entry_impl(ptr: *mut u32, idx: usize, entry: TrapEntry) {
    let pc = unsafe { ptr.offset(idx as isize) };

    unsafe { *pc = entry.to_jal(pc as usize).unwrap() };
}

#[cfg(test)]
mod test {
    use super::create_jal;

    #[test]
    pub fn test_create_jal() {
        let pc = 0x80002b40usize;
        let target = 0x80008914usize;

        let instr = create_jal(target, pc);
        let real_instr: u32 = 0x5d50506f;
        assert_eq!(instr, real_instr);

        let pc = 0x8010002cusize;
        let target = 0x80100000usize;
        let instr = create_jal(target, pc);
        let real_instr: u32 = 0xfd5ff06f;
        assert_eq!(instr, real_instr);
    }
}
