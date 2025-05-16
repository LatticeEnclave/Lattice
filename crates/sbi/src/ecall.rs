use core::arch::asm;

/// 	unsigned long extension_id = regs->a7;
///	unsigned long func_id = regs->a6;
/// ext id: 0x48534D
/// #define SBI_EXT_HSM_HART_START			0x0
/// #define SBI_EXT_HSM_HART_STOP			0x1
/// #define SBI_EXT_HSM_HART_GET_STATUS		0x2
/// #define SBI_EXT_HSM_HART_SUSPEND		0x3

const SBI_EXT_HSM: usize = 0x48534D;
const SBI_EXT_HSM_HART_START: usize = 0x0;
const SBI_EXT_HSM_HART_STOP: usize = 0x1;
const SBI_EXT_HSM_HART_GET_STATUS: usize = 0x2;

const SBI_EXT_IPI: usize = 0x735049;
const SBI_EXT_IPI_SEND_IPI: usize = 0x0;

pub const SBI_EXT_HTEE_ENCLAVE: usize = 0x08abcdef;

// Enclave stop reasons requested
pub const STOP_TIMER_INTERRUPT: usize = 0;
pub const STOP_PROXY_CALL_HOST: usize = 1;
pub const STOP_EXIT_ENCLAVE: usize = 2;

pub enum RuntimeSbiCall {
    RuntimeSyscallUnknown = 1000,
    RuntimeSyscallOcall = 1001,
    RuntimeSyscallSharedcopy = 1002,
    RuntimeSyscallAttestEnclave = 1003,
    RuntimeSyscallGetSealingKey = 1004,
    RuntimeSyscallExit = 1101,
}

pub enum SBISMEnclaveCall {
    SbiSMCreateEnclave = 2001,
    SbiSMDestroyEnclave = 2002,
    SbiSMRunEnclave = 2003,
    SbiSMResumeEnclave = 2005,
    SbiSMRandom = 3001,
    SbiSmAttestEnclave = 3002,
    SbiSMGetSealingKey = 3003,
    SbiSMStopEnclave = 3004,
    SbiSMExitEnclave = 3006,
    SbiSMEneterLde = 3007,
    SbiSMExitLde = 3008,
    SbiSMCallPlugin = 4000,
    SbiSMELock = 5001,
    SbiSMEFree = 5002,
    SbiSMEnclaveCtl = 5003,
    SbiSMEnclaveFinishCtl = 5004,
    SbiSMChannelOpen = 5005,
    SbiSMChannelConnect = 5006,
    SbiSMChannelClose = 5007,
    SbiSMCopyFromLue = 5008,
    SbiSMCopyToLue = 5009,
    SbiSMCopyFromKernel = 5010,
    SbiSMCopyToKernel = 5011,
}

pub mod pmu {
    pub const SBI_EXT_PMU: usize = 0x504D55;

    const SBI_PMU_EVENT_IDX_TYPE_OFFSET:usize = 16;
    use bitflags::bitflags;

    pub enum PmuFunc {
        Num = 0x0,
        GetInfo = 0x1,
        CfgMatch = 0x2,
        Start = 0x3,
        Stop = 0x4,
        FwRead = 0x5,
    }

    enum SbiPmuEventTypeId {
        SbiPmuEventTypeHw				= 0x0,
        SbiPmuEventTypeHwCache			= 0x1,
        SbiPmuEventTypeHwRaw			= 0x2,
        SbiPmuEventTypeFw				= 0xf,
        SbiPmuEventTypeMax,
    }

    /**
     * Special "firmware" events provided by the OpenSBI, even if the hardware
     * does not support performance events. These events are encoded as a raw
     * event type in Linux kernel perf framework.
     */
    enum SbiPmuFwEventCodeId {
        SbiPmuFwMisalignedLoad	= 0,
        SbiPmuFwMisalignedStore	= 1,
        SbiPmuFwAccessLoad		= 2,
        SbiPmuFwAccessStore		= 3,
        SbiPmuFwIllegalInsn		= 4,
        SbiPmuFwSetTimer		= 5,
        SbiPmuFwIpiSent		= 6,
        SbiPmuFwIpiRecvd		= 7,
        SbiPmuFwFenceISent		= 8,
        SbiPmuFwFenceIRecvd	= 9,
        SbiPmuFwSfenceVmaSent	= 10,
        SbiPmuFwSfenceVmaRcvd	= 11,
        SbiPmuFwSfenceVmaAsidSent	= 12,
        SbiPmuFwSfenceVmaAsidRcvd	= 13,

        SbiPmuFwHfenceGvmaSent	= 14,
        SbiPmuFwHfenceGvmaRcvd	= 15,
        SbiPmuFwHfenceGvmaVmidSent = 16,
        SbiPmuFwHfenceGvmaVmidRcvd = 17,

        SbiPmuFwHfenceVvmaSent	= 18,
        SbiPmuFwHfenceVvmaRcvd	= 19,
        SbiPmuFwHfenceVvmaAsidSent = 20,
        SbiPmuFwHfenceVvmaAsidRcvd = 21,
        SbiPmuFwMax,
        /*
        * Event codes 22 to 255 are reserved for future use.
        * Event codes 256 to 65534 are reserved for SBI implementation
        * specific custom firmware events.
        */
        SbiPmuFwReservedMax = 0xFFFE,
        /*
        * Event code 0xFFFF is used for platform specific firmware
        * events where the event data contains any event specific information.
        */
        SbiPmuFwPlatform = 0xFFFF,
    }

    /** General pmu event codes specified in SBI PMU extension */
    enum SbiPmuHwGenericEventsT {
        SbiPmuHwNoEvent			= 0,
        SbiPmuHwCpuCycles			= 1,
        SbiPmuHwInstructions			= 2,
        SbiPmuHwCacheReferences		= 3,
        SbiPmuHwCacheMisses			= 4,
        SbiPmuHwBranchInstructions		= 5,
        SbiPmuHwBranchMisses		= 6,
        SbiPmuHwBusCycles			= 7,
        SbiPmuHwStalledCyclesFrontend	= 8,
        SbiPmuHwStalledCyclesBackend	= 9,
        SbiPmuHwRefCpuCycles		= 10,

        SbiPmuHwGeneralMax,
    }

    pub fn gen_cpu_cycle_event_idx() -> usize {
        ((SbiPmuEventTypeId::SbiPmuEventTypeHw as usize) << SBI_PMU_EVENT_IDX_TYPE_OFFSET ) | SbiPmuHwGenericEventsT::SbiPmuHwCpuCycles as usize
    }

    pub fn gen_inst_event_idx() -> usize {
        ((SbiPmuEventTypeId::SbiPmuEventTypeHw as usize) << SBI_PMU_EVENT_IDX_TYPE_OFFSET ) | SbiPmuHwGenericEventsT::SbiPmuHwInstructions as usize
    }

    bitflags! {
        /// match and comfigure flag
        pub struct CfgFlag: u8 {
            const SBI_PMU_CFG_FLAG_SKIP_MATCH = 1 << 0;     // skip match, directly configure
            const SBI_PMU_CFG_FLAG_CLEAR_VALUE = 1 << 1;    // clear the counter
            const SBI_PMU_CFG_FLAG_AUTO_START = 1 << 2;     // start after configure atomatically
            const SBI_PMU_CFG_FLAG_SET_VUINH = 1 << 3;      // U-Mode visibility
            const SBI_PMU_CFG_FLAG_SET_VSINH = 1 << 4;      // S-Mode visibility
            const SBI_PMU_CFG_FLAG_SET_UINH = 1 << 5;       // aggragate U-Mode 
            const SBI_PMU_CFG_FLAG_SET_SINH = 1 << 5;       // aggragate S-Mode    
            const SBI_PMU_CFG_FLAG_SET_MINH = 1 << 7;       // aggragate M-Mode
        }
    }

    impl CfgFlag {
        pub fn MSU() -> CfgFlag {
            CfgFlag::SBI_PMU_CFG_FLAG_CLEAR_VALUE | CfgFlag::SBI_PMU_CFG_FLAG_SET_UINH | CfgFlag::SBI_PMU_CFG_FLAG_SET_SINH | CfgFlag::SBI_PMU_CFG_FLAG_SET_MINH
        }
    }

    bitflags! {
        /// match and comfigure flag
        pub struct StartFlag: u8 {
            const SBI_PMU_START_FLAG_SET_INIT_VALUE = 1 << 0;   // start from initial value
            const SBI_PMU_START_FLAG_INIT_FROM_SNAPSHOT = 1 << 1;   // no need
        }
    }

    impl StartFlag {
        pub fn IVAL() -> StartFlag {
            StartFlag::SBI_PMU_START_FLAG_SET_INIT_VALUE
        }
    }

    bitflags! {
        /// match and comfigure flag
        pub struct StopFlag: u8 {
            const SBI_PMU_STOP_FLAG_RESET = 1 << 0;     // reset the counter
            const SBI_PMU_STOP_FLAG_TAKE_SNAPSHOT = 1 << 1; // no need
        }
    }

    impl StopFlag {
        pub fn RST() -> StopFlag {
            StopFlag::SBI_PMU_STOP_FLAG_RESET
        }
    }

    pub fn get_num() -> usize {
        let (ret, num) = num();
        num
    }


    /// now we only need cycle, instrucions and time
    /// the codes below show how to apply sbi pmu counters
    // /// the next two counter is fixed in hardware without SBI_HART_EXT_SSCOFPMF(in this condition, cbase and cmask are no need)
    // pub fn configure_cpu_cycle_counter() -> usize {
    //     let (ret, idx) = CfgMatch(0x0, 0x1f, CfgFlag::MSU().bits() as usize, gen_cpu_cycle_event_idx(), 0);
    //     idx
    // }

    // pub fn configure_inst_counter() -> usize {
    //     let (ret, idx) = CfgMatch(0x0, 0x1f, CfgFlag::MSU().bits() as usize, gen_inst_event_idx(), 0);
    //     idx
    // }

    // pub fn start_cycle_inst_counter(idx1: usize, idx2: usize) -> isize {
    //     let ret = start(0, (1 << idx1) | (1 << idx2), StartFlag::IVAL().bits() as usize, 0);

    //     ret
    // }

    // pub fn stop_cycle_inst_counter(idx1: usize, idx2: usize) -> isize {
    //     let ret = stop(0, (1 << idx1) | (1 << idx2), 0);

    //     ret
    // }

    /// get the counter numbers
    /// out:
    ///     a0: ret
    ///     a1: number
    pub fn num() -> (isize, usize) {
        let ret: isize;
        let out: usize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a6") PmuFunc::Num as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") ret,
                lateout("a1") out,
            )
        }

        (ret, out)
    }

    /// get the pmu n idx counter info 
    /// in :
    ///     a0: idx
    /// out:
    ///     a0: ret
    ///     a1: info
    /// union sbi_pmu_ctr_info {
    /// 	unsigned long value;
    /// 	struct {
    /// 		unsigned long csr:12;
    /// 		unsigned long width:6;
    ///  #if __riscv_xlen == 32
    /// 		unsigned long reserved:13;
    ///  #else
    ///  		unsigned long reserved:45;
    ///  #endif
    ///  		unsigned long type:1;
    ///  	};
    ///  };
    pub fn GetInfo(a0: usize) -> (isize, usize) {
        let ret: isize;
        let out: usize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") a0,                
                in("a6") PmuFunc::GetInfo as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") ret,
                lateout("a1") out,
            )
        }

        (ret, out)
    }


    /// match and configure the counter
    /// in:
    ///     a0: idx
    ///     a1: idx mask(bitmap 0~31)
    ///     a2: configure flag
    ///     a3: event_idx
    ///     a4: event_data(no need now)
    /// out:
    ///     a0: ret
    ///     a1: configured idx
    pub fn CfgMatch(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> (isize, usize) {
        let rc: isize;
        let idx: usize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") a0,
                in("a1") a1,
                in("a2") a2,
                in("a3") a3,
                in("a4") a4,            
                in("a6") PmuFunc::CfgMatch as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") rc,
                lateout("a1") idx,
            )
        }
        (rc, idx)
    }

    /// start counters
    /// in:
    ///     a0: idx
    ///     a1: idx mask(bitmap 0~31)
    ///     a2: start flag
    ///     a3: initial value
    /// out:
    ///     a0: ret
    pub fn start(a0: usize, a1: usize, a2: usize, a3: usize) -> isize {
        let rc: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") a0,
                in("a1") a1,
                in("a2") a2,
                in("a3") a3,
                in("a6") PmuFunc::Start as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") rc,
            )
        }

        rc
    }

    /// stop counters
    /// in:
    ///     a0: idx
    ///     a1: idx mask(bitmap 0~31)
    ///     a2: stop flag
    /// out:
    ///     a0: ret
    pub fn stop(a0: usize, a1: usize, a2: usize) -> isize {
        let rc: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") a0,
                in("a1") a1,
                in("a2") a2,
                in("a6") PmuFunc::Stop as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") rc,
            )
        }

        rc
    }

    /// read counter in idx
    /// in:
    ///     a0: idx
    /// out:
    ///     a0: ret
    ///     a1: value
    pub fn fw_read(a0: usize) -> (isize, usize) {
        let ret: isize;
        let out: usize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") a0,
                in("a6") PmuFunc::FwRead as usize,
                in("a7") SBI_EXT_PMU,
                lateout("a0") ret,
                lateout("a1") out,
            )
        }

        (ret, out)
    }
}
pub fn runtime_sbi_call_from_usize(n: usize) -> Option<RuntimeSbiCall> {
    match n {
        1000 => Some(RuntimeSbiCall::RuntimeSyscallUnknown),
        1001 => Some(RuntimeSbiCall::RuntimeSyscallOcall),
        1002 => Some(RuntimeSbiCall::RuntimeSyscallSharedcopy),
        1003 => Some(RuntimeSbiCall::RuntimeSyscallAttestEnclave),
        1004 => Some(RuntimeSbiCall::RuntimeSyscallGetSealingKey),
        1101 => Some(RuntimeSbiCall::RuntimeSyscallExit),
        _ => None,
    }
}

pub fn enclave_sbi_call_from_usize(n: usize) -> Option<SBISMEnclaveCall> {
    match n {
        2001 => Some(SBISMEnclaveCall::SbiSMCreateEnclave),
        2002 => Some(SBISMEnclaveCall::SbiSMDestroyEnclave),
        2003 => Some(SBISMEnclaveCall::SbiSMRunEnclave),
        2005 => Some(SBISMEnclaveCall::SbiSMResumeEnclave),
        3001 => Some(SBISMEnclaveCall::SbiSMRandom),
        3002 => Some(SBISMEnclaveCall::SbiSmAttestEnclave),
        3003 => Some(SBISMEnclaveCall::SbiSMGetSealingKey),
        3004 => Some(SBISMEnclaveCall::SbiSMStopEnclave),
        3006 => Some(SBISMEnclaveCall::SbiSMExitEnclave),
        4000 => Some(SBISMEnclaveCall::SbiSMCallPlugin),
        _ => None,
    }
}

#[inline(never)]
pub fn sbi_hsm_hart_start_ecall(hartid: usize, addr: usize, arg1: usize) -> isize {
    let rc: isize;
    unsafe {
        asm!(
            "ecall",
            in("a0") hartid,
            in("a1") addr,
            in("a2") arg1,
            in("a6") SBI_EXT_HSM_HART_START,
            in("a7") SBI_EXT_HSM,
            lateout("a0") rc,
        )
    }
    rc
}

#[inline(never)]
pub fn sbi_hsm_hart_get_state_ecall(hartid: usize) -> isize {
    let rc: isize;
    unsafe {
        asm!(
            "ecall",
            in("a0") hartid,
            in("a6") SBI_EXT_HSM_HART_GET_STATUS,
            in("a7") SBI_EXT_HSM,
            lateout("a0") rc,
        )
    }
    rc
}

pub fn sbi_hsm_hart_stop_ecall(hartid: usize) -> isize {
    let rc: isize;
    unsafe {
        asm!(
            "ecall",
            in("a0") hartid,
            in("a6") SBI_EXT_HSM_HART_STOP,
            in("a7") SBI_EXT_HSM,
            lateout("a0") rc
        )
    }
    rc
}

#[inline(never)]
pub fn sbi_ecall_send_ipi(hmask: usize, hbase: isize) -> isize {
    let rc: isize;
    unsafe {
        asm!(
            "ecall",
            in("a0") hmask,
            in("a1") hbase,
            in("a6") SBI_EXT_IPI_SEND_IPI,
            in("a7") SBI_EXT_IPI,
            lateout("a0") rc
        )
    }
    rc
}

#[inline(never)]
pub fn sbi_call_1(eid: usize, fid: usize, arg0: usize) -> (isize, isize) {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            inlateout("a0") arg0 => error,
            lateout("a1") value,
        );
    }
    (error, value)
}

#[inline(never)]
pub fn sbi_unimp_1(eid: usize, fid: usize, arg0: usize) -> (isize, isize) {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "unimp",
            in("a7") eid,
            in("a6") fid,
            inlateout("a0") arg0 => error,
            lateout("a1") value,
        );
    }
    (error, value)
}

#[inline(never)]
pub fn sbi_unimp_2(eid: usize, fid: usize, arg0: usize, arg1: usize) -> (isize, isize) {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "unimp",
            in("a7") eid,
            in("a6") fid,
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => value,
        );
    }
    (error, value)
}

#[inline(never)]
pub fn sbi_unimp_3(
    eid: usize,
    fid: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
) -> (isize, isize) {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "unimp",
            in("a7") eid,
            in("a6") fid,
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => value,
            in("a2") arg2,
        );
    }
    (error, value)
}

#[inline(never)]
pub fn sbi_exit_enclave(retval: usize) -> (isize, isize) {
    sbi_call_1(
        SBI_EXT_HTEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMExitEnclave as usize,
        retval,
    )
}
