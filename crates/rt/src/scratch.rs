use core::ptr::NonNull;
use core::{arch::asm, marker::PhantomPinned};

use riscv::register::sscratch;

use crate::{
    consts::{MAX_HART_NUM, PAGE_SIZE, STACK_SIZE},
    kernel::{LinuxUserKernel, LinuxDriverKernel, KernelPtr},
    stack::Stack,
};

pub fn get_scratch_mut() -> &'static mut HartScratch {
    todo!()
}

#[repr(C)]
pub struct Scratch {
    pub t0: usize,
    pub kernel: KernelPtr,
    pub prev_satp: usize,
}

#[repr(C)]
pub struct HartScratch {
    stack: Stack<STACK_SIZE>,
    scratch: Scratch,
}

#[repr(C)]
pub struct ScratchManager {
    scratches: [HartScratch; MAX_HART_NUM],
    _pinned: PhantomPinned,
}

impl ScratchManager {
    pub unsafe fn uninit_at(addr: usize) -> &'static mut Self {
        &mut *(addr as *mut Self)
    }

    //创建新实例分配空间
    pub fn init_all(&mut self, kernel: KernelPtr) {
        for i in 0..MAX_HART_NUM {
            *self.get_scratch_mut(i) = Scratch {
                t0: 0,
                kernel: kernel,
                prev_satp: 0,
            };
        }
    }

    pub fn get_scratch(&self, hartid: usize) -> &Scratch {
        &self.scratches[hartid].scratch
    }

    pub fn get_scratch_mut(&mut self, hartid: usize) -> &mut Scratch {
        &mut self.scratches[hartid].scratch
    }
    pub fn get_hartid(&self, scratch: &Scratch) -> usize {
        self.scratches
            .iter()
            .position(|hart_scratch| &hart_scratch.scratch as *const _ == scratch as *const _)
            .expect("Scratch instance not found in ScratchManager")
    }
}

//切换栈修改寄存器
pub fn switch_scratch(manager: &ScratchManager, hartid: usize) {
    let sscratch = manager.get_scratch(hartid) as *const Scratch as usize;
    unsafe {
        asm!(
            "csrw sscratch, {0}",
            in(reg) sscratch,
        );
    }
}

impl Scratch {
    pub unsafe fn from_ssratch() -> &'static Scratch {
        let addr = sscratch::read();
        NonNull::new(addr as *mut Scratch).unwrap().as_mut()
    }

    pub fn get_lu_kernel(&self) -> &LinuxUserKernel {
        match self.kernel {
            KernelPtr::LUKPtr(luk_ptr) => unsafe { luk_ptr.as_ref() },
            _ => panic!()
        }
    }

    pub fn get_ld_kernel(&self) -> &LinuxDriverKernel {
        match self.kernel {
            KernelPtr::LDKPtr(ldk_ptr) => unsafe { ldk_ptr.as_ref() },
            _ => panic!()
        }
    }
}
