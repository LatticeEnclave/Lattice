pub mod client {
    use core::arch::asm;
    #[inline(always)]
    pub fn resume_enclave(eidx: usize) -> (usize, usize) {
        let rc0;
        let rc1;
        unsafe {
            asm!(
                "unimp",
                in("a0") eidx,
                in("a6") sbi::ecall::SBISMEnclaveCall::SbiSMResumeEnclave as usize,
                in("a7") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
                lateout("a0") rc0,
                lateout("a1") rc1,
                lateout("a6") _,
                lateout("a7") _,
                options(nostack)
            )
        }
        (rc0, rc1)
    }

    use crate::info::LueInfo;

    #[inline(never)]
    pub fn create_lue(info: *const LueInfo) -> (usize, usize) {
        let rc;
        let eidx;
        unsafe {
            asm!(
                "unimp",
                in("a0") info,
                in("a1") 1,
                in("a6") sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize,
                in("a7") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
                lateout("a0") rc,
                lateout("a1") eidx,
                lateout("a6") _,
                lateout("a7") _,
                options(nostack)
            )
        }

        (rc, eidx)
    }

    use crate::info::LseInfo;
    #[inline(never)]
    pub fn create_lse(info: *const LseInfo) -> (usize, usize) {
        let rc;
        let eidx;
        unsafe {
            asm!(
                "unimp",
                in("a0") info,
                in("a1") 3,
                in("a6") sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize,
                in("a7") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
                lateout("a0") rc,
                lateout("a1") eidx,
                lateout("a6") _,
                lateout("a7") _,
                options(nostack)
            )
        }

        (rc, eidx)
    }

    use crate::info::LdeInfo;
    #[inline(never)]
    pub fn create_lde(info: *const LdeInfo) -> usize {
        let rc;
        unsafe {
            asm!(
                "unimp",
                in("a0") info,
                in("a1") 2,
                in("a6") sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize,
                in("a7") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
                lateout("a0") rc,
                lateout("a6") _,
                lateout("a7") _,
                options(nostack)
            )
        }

        rc
    }

    #[inline(always)]
    pub fn launch_enclave(eidx: usize) -> (usize, usize) {
        let a0: usize;
        let a1: usize;
        unsafe {
            asm!(
                "unimp",
                in("a0") eidx,
                in("a6") sbi::ecall::SBISMEnclaveCall::SbiSMRunEnclave as usize,
                in("a7") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
                lateout("a0") a0,
                lateout("a1") a1,
                lateout("a6") _,
                lateout("a7") _,
                options(nostack)
            )
        }

        (a0, a1)
    }

    // pub fn request_ctl(head: *const usize) {
    //     unsafe {
    //         asm!(
    //             "ecall",
    //             in("a0") head,
    //             in("a6") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
    //             in("a7") sbi::ecall::SBISMEnclaveCall::SbiSMEnclaveCtl as usize,
    //             options(nostack)
    //         )
    //     }
    // }

    // pub fn finish_ctl(head: *const usize, rc: isize) -> isize {
    //     unsafe {
    //         asm!(
    //             "ecall",
    //             in("a0") head,
    //             in("a1") rc,
    //             in("a6") sbi::ecall::SBI_EXT_HTEE_ENCLAVE,
    //             in("a7") sbi::ecall::SBISMEnclaveCall::SbiSMEnclaveFinishCtl as usize,
    //             options(nostack)
    //         )
    //     }

    //     return rc;
    // }
}

pub mod runtime {
    use core::fmt::Display;

    use htee_device::device::Device;

    pub use elf::Sections;

    pub struct LdeBootArgs {
        pub mem: MemArg,
        pub mods: ModArg,
        pub tp: TpArg,
        pub bin: BinArg,
        pub unmapped: UnmappedArg,
        pub driver_start: usize,
        pub driver_size: usize,
        pub sections: Sections,
        pub device: Device,
    }

    impl Display for LdeBootArgs {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "
{}
{}
{}
{}
driver start: {:#x}, size: {:#x}
{}
",
                self.mem,
                self.mods,
                self.tp,
                self.bin,
                self.driver_start,
                self.driver_size,
                self.unmapped
            ))
        }
    }

    pub struct LueBootArgs {
        pub mem: MemArg,
        pub mods: ModArg,
        pub tp: TpArg,
        pub bin: BinArg,
        pub shared: SharedArg,
        pub unmapped: UnmappedArg,
        pub device: Device,
    }

    impl Display for LueBootArgs {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "
{}
{}
{}
{}
{}
{}
",
                self.mem, self.mods, self.tp, self.bin, self.shared, self.unmapped
            ))
        }
    }

    pub struct MemArg {
        pub total_size: usize,
    }

    impl Display for MemArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!("total memory size: {:#x}", self.total_size))
        }
    }

    pub struct ModArg {
        pub start_vaddr: usize,
        pub num: usize,
    }

    impl Display for ModArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "{} modules. First module address: {:#x}",
                self.num, self.start_vaddr
            ))
        }
    }

    pub struct TpArg {
        pub addr: usize,
    }

    impl Display for TpArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!("trampoline address: {:#x}", self.addr))
        }
    }

    pub struct BinArg {
        pub start: usize,
        pub size: usize,
    }

    impl Display for BinArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "binary address: {:#x}, size: {:#x}",
                self.start, self.size
            ))
        }
    }

    #[derive(Clone, Copy)]
    pub struct SharedArg {
        pub enc_vaddr: usize,
        pub host_vaddr: usize,
        pub size: usize,
    }

    impl Display for SharedArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "shared memory pool host address: {:#x}, enclave address: {:#x}, size: {:#x}",
                self.host_vaddr, self.enc_vaddr, self.size
            ))
        }
    }

    pub struct UnmappedArg {
        pub head: usize,
        pub size: usize,
    }

    impl Display for UnmappedArg {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "The head of unused memory at physical address {:#x}, the size: {:#x}",
                self.head, self.size,
            ))
        }
    }
}
