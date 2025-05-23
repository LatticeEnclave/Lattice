use core::{ops::Range, sync::atomic::AtomicPtr};

use fdt::Fdt;
use hsm::MAX_HART_NUM;
use htee_device::device::{DeviceInfo, MemRegion};
use riscv::register::Permission;
use sbi::TrapRegs;
use spin::Once;
use trap_proxy::ProxyResult;

// pub struct Platform {
//     pub pmp_count: usize,
//     pub sbi_start: usize,
//     pub heap_size: usize,
//     pub sm_rw_size: usize,

//     pub ops: PlatformOps,
// }

// pub struct PlatformOps {
//     pub get_sbi_region: fn() -> Range<usize>,
//     pub get_heap_region: fn() -> Range<usize>,
//     pub get_pma_region: fn() -> Range<usize>,
//     pub get_hart_num: fn() -> usize,
// }

// impl Default for PlatformOps {
//     fn default() -> Self {}
// }

// pub fn get_sbi_region() -> Range<usize> {
//     // let info = DeviceInfo::new(fdt).unwrap();
//     // todo!()
//     let sbi_start = Self::SBI_START;
//     let mut sbi_text_end = 0;
//     let mut sbi_rw_end = 0;
//     // pmp::iter_hps().filter_map(|)
//     for pmp in pmp::iter_hps() {
//         if pmp.is_off() {
//             continue;
//         }
//         if pmp.region.start == sbi_start && pmp.permission == Permission::NONE {
//             sbi_text_end = pmp.region.end;
//             break;
//         }
//     }

//     for pmp in pmp::iter_hps() {
//         if pmp.is_off() {
//             continue;
//         }
//         if pmp.region.start == sbi_text_end && pmp.permission == Permission::NONE {
//             sbi_rw_end = pmp.region.end;
//             break;
//         } else if pmp.region.start == sbi_start
//             && pmp.permission == Permission::NONE
//             && pmp.region.end > sbi_rw_end
//         {
//             sbi_rw_end = pmp.region.end;
//         }
//     }

//     sbi_start..sbi_rw_end
// }

pub trait Platform {
    const PMP_COUNT: usize = 16;
    const SBI_START: usize = 0x0;
    const HEAP_SIZE: usize = 0x10000;
    const SM_RW_SIZE: usize = 0x40000;

    // fn init(fdt: usize) {}

    // unsafe fn trap_entry() {
    //     struct Handler;

    //     impl trap_proxy::TrapProxy for Handler {
    //         fn handle(regs: &mut TrapRegs) -> ProxyResult {

    //         }
    //     }
    //     unsafe { trap_proxy::TrapProxy::_enter() };
    // }

    // fn trap_handler(regs: &mut TrapRegs) -> ProxyResult;

    fn get_sbi_region(&self) -> Range<usize> {
        // let info = DeviceInfo::new(fdt).unwrap();
        // todo!()
        let sbi_start = Self::SBI_START;
        let mut sbi_text_end = 0;
        let mut sbi_rw_end = 0;
        // pmp::iter_hps().filter_map(|)
        for pmp in pmp::iter_hps() {
            if pmp.is_off() {
                continue;
            }
            if pmp.region.start == sbi_start && pmp.permission == Permission::NONE {
                sbi_text_end = pmp.region.end;
                break;
            }
        }

        for pmp in pmp::iter_hps() {
            if pmp.is_off() {
                continue;
            }
            if pmp.region.start == sbi_text_end && pmp.permission == Permission::NONE {
                sbi_rw_end = pmp.region.end;
                break;
            } else if pmp.region.start == sbi_start
                && pmp.permission == Permission::NONE
                && pmp.region.end > sbi_rw_end
            {
                sbi_rw_end = pmp.region.end;
            }
        }

        sbi_start..sbi_rw_end
    }

    fn get_heap_region(&self) -> Range<usize> {
        let sbi_region = self.get_sbi_region();
        sbi_region.end..(sbi_region.end + Self::HEAP_SIZE)
    }

    fn get_pma_region(&self) -> Range<usize> {
        // let sbi_region = self.get_sbi_region();
        let heap_region = self.get_heap_region();
        heap_region.end..(heap_region.end + Self::SM_RW_SIZE)
    }

    #[inline]
    fn get_hart_num(&self) -> usize {
        let mut hart_num = 0;
        for i in 0..MAX_HART_NUM {
            let rc = sbi::ecall::sbi_hsm_hart_get_state_ecall(i);
            if rc < 0 {
                break;
            }
            hart_num += 1;
        }
        hart_num
    }

    // fn trap_entry(regs: &mut TrapRegs);

    // #[inline]
    // fn init_heap(&mut self, start: *mut u8, size: usize) {
    //     use pmp::PmpBuf;

    //     type BufPool = Vec<PmpBuf, MAX_HART_NUM>;

    //     assert!(aligned!(start as usize, 0x8));
    //     assert!(core::mem::size_of::<BufPool>() <= size);
    //     let mut ptr = NonNull::new(start as *mut BufPool).unwrap();
    //     unsafe {
    //         *ptr.as_mut() = Vec::new();
    //         for (i, hs) in self.hsm.iter_hs_mut().enumerate() {
    //             ptr.as_mut().push(Vec::new()).unwrap();
    //             hs.pmp_buf = NonNull::new(ptr.as_mut().get_mut(i).unwrap()).unwrap()
    //         }
    //     }
    // }
}

// pub struct Platform {

// }
