use core::arch::asm;

use crate::consts::RUNTIME_VA_START;

#[inline(never)] // never inline
#[link_section = ".tp"]
pub unsafe extern "C" fn readp_usize(addr: usize) -> usize {
    let val: usize;
    asm!(
        "csrrw  t0, satp, x0", // disable satp by set satp = 0
        "ld     a0, 0(a0)",
        "csrw   satp, t0",
        inout("a0") addr => val,
        out("t0") _,
        options(nostack)
    );
    val
}

#[inline(never)] // never inline
#[link_section = ".tp"]
pub unsafe extern "C" fn writep_usize(addr: usize, val: usize) {
    asm!(
        "csrrw  t0, satp, x0",
        "sd     a1, 0(a0)",
        "csrw   satp, t0",
        in("a0") addr,
        in("a1") val,
        out("t0") _,
        options(nostack)
    );
}

#[derive(Clone)]
pub struct Trampoline {
    read_fp: usize,
    write_fp: usize,
}

impl Trampoline {
    /// Record the trampoline address.
    ///
    /// Note: this function should only called once before remap.
    pub fn create(trampoline: usize) -> Self {
        Self {
            read_fp: readp_usize as usize - RUNTIME_VA_START + trampoline,
            write_fp: writep_usize as usize - RUNTIME_VA_START + trampoline,
        }
    }

    pub unsafe fn read_ref<T: From<usize>>(&self, target: &T) -> T {
        self.readp(target as *const T as usize)
    }

    pub unsafe fn readp<T: From<usize>, U: Into<usize>>(&self, addr: U) -> T {
        let func = core::mem::transmute::<usize, fn(usize) -> usize>(self.read_fp);
        let val = func(addr.into());

        val.into()
    }

    pub unsafe fn write_mut<T: Into<usize>>(&self, target: &mut T, val: T) {
        self.writep(target as *mut T as usize, val)
    }

    pub unsafe fn writep<T: Into<usize>, U: Into<usize>>(&self, addr: U, val: T) {
        let func = core::mem::transmute::<usize, fn(usize, usize)>(self.write_fp);
        func(addr.into(), val.into());
    }
}
