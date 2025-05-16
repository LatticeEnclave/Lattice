use core::{
    arch::asm,
    mem::MaybeUninit,
    ops::Range,
    panic, slice,
    sync::atomic::{AtomicBool, Ordering},
    usize,
};
use htee_console::{init_console_uart, log, println};
use htee_device::device::{DeviceInfo, MemRegion};
use htee_macro::usize_env_or;
use pmp::{calc_napot_area, PmpRegGroup, PMP_COUNT};
// use pma::Owner;
use riscv::register::{
    mepc, mhartid, mscratch, mstatus, mtvec, satp, sie,
    stvec::{self, TrapMode},
    Permission,
};
use sbi::ecall::sbi_hsm_hart_get_state_ecall;
use sm::{
    check_stack_overflow, consts::MAX_HART_NUM, sm, Owner, PhysMemArea, PhysMemAreaMgr, PmaProp,
    PmpStatus, SecMonitor, SM,
};

use crate::trap::{direct, tmp};

const FW_TEXT_START: usize = usize_env_or!("FW_TEXT_START", 0);
// const SBI_SIZE: usize = usize_env_or!("SBI_SIZE", 0x60000);
// const SM_TEXT_START: usize = usize_env_or!("SM_TEXT_START", 0x60000);
// const REV_SIZE: usize = SM_RW_SIZE;
// const SM_SIZE: usize = 0x40000;
// const SM_RW_START: usize = usize_env_or!("SM_RW_START", 0x80100000);
const SM_RW_SIZE: usize = 0x40000;
const SM_HEAP_SIZE: usize = 0x10000;

pub unsafe fn sm_init(next_addr: usize, arg1: usize) -> ! {
    static IS_COLD: AtomicBool = AtomicBool::new(true);

    let hartid = mhartid::read();

    // cold boot hart, arg0 = hartid, arg1 = fdt
    if IS_COLD.swap(false, Ordering::AcqRel) {
        boothart_init(next_addr, arg1);
    }

    check_stack_overflow();
    common_init(next_addr, arg1)
}

#[inline(never)]
unsafe fn boothart_init(next_addr: usize, fdt: usize) {
    let mut device_info = DeviceInfo::new(fdt as *const u8).unwrap();

    init_console_uart(device_info.get_uart().unwrap());

    log::debug!("SM console inited");

    log::debug!("fdt: {fdt:#x}");

    log::debug!("PMP COUNT: {}", PMP_COUNT);

    // #[cfg(debug_assertions)]
    // for node in device_info.iter_all_nodes() {
    //     log::debug!("node: {}", node.name);
    // }

    // if let Some(dma) = device_info.get_dma() {
    //     log::debug!("dma: {:#x}", dma as usize);
    // }

    let rw_start = init_device(&mut device_info);

    log::debug!("{} do init", mhartid::read());

    // initialize the memory region, and update the reserved memory area
    create_sm(device_info, rw_start);

    // update sbi trap handler
    let sbi_handler = mtvec::read().address();
    direct::init_handler(sbi_handler);
}

unsafe fn common_init(next_addr: usize, arg1: usize) -> ! {
    let hartid = mhartid::read();
    log::debug!("{} start", hartid);

    // update hart pmp, clean all permission
    init_hart_pmp();
    log::debug!("inited hart {hartid} pmp");

    // let old_handler = mtvec::read().address();
    mtvec::write(direct::trap_handler as usize, TrapMode::Direct);
    // let new_handler = mtvec::read().address();
    // log::debug!("updated hart {hartid} mtvec from {old_handler:#x} to {new_handler:#x}",);

    log::debug!("mret to {next_addr:#x}");
    // log::debug!("mscratch: {:#x}", mscratch::read());

    unsafe { jump_next_s_mode(hartid, arg1, next_addr) }
}

/// 清除 mstatus 寄存器的 MPIE 位（Machine Previous Interrupt Enable）
/// # 安全性
/// - 该函数必须在机器模式（Machine Mode）下调用，否则会触发非法指令异常。
/// - 直接操作 CSR 可能破坏系统状态，需确保上下文安全。
#[inline(always)]
pub unsafe fn clear_mstatus(offset: usize) {
    // RISC-V 中 MPIE 位的位置（固定为第 7 位）
    let mask: usize = 1 << offset;

    // 读取当前 mstatus 寄存器的值
    let mut mstatus: usize;
    unsafe {
        asm!(
            "csrr {0}, mstatus", // 使用 csrr 指令读取 mstatus
            out(reg) mstatus,
            options(nostack, nomem)
        );
    }

    // 清除 MPIE 位
    mstatus &= !mask;

    // 将修改后的值写回 mstatus
    unsafe {
        asm!(
            "csrw mstatus, {0}", // 使用 csrw 指令写入 mstatus
            in(reg) mstatus,
            options(nostack, nomem)
        );
    }
}

unsafe fn jump_next_s_mode(arg0: usize, arg1: usize, next_addr: usize) -> ! {
    stvec::write(next_addr, TrapMode::Direct);
    mstatus::set_mpp(mstatus::MPP::Supervisor);
    mepc::write(next_addr);

    asm!(
        "mret",
        in("a0") arg0,
        in("a1") arg1,
        options(noreturn)
    )
}

/// Clean all pmp registers and disable rwx
#[inline]
fn init_hart_pmp() {
    for i in 0..PMP_COUNT {
        unsafe { PmpStatus::new().off().apply(i) };
    }

    unsafe {
        PmpStatus::from_register(0)
            .region(0..0x1000)
            .napot()
            .permission(riscv::register::Permission::NONE)
            .apply(0);
    };
}

#[inline]
fn get_sbi_region() -> Range<usize> {
    let sbi_start = FW_TEXT_START;
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

fn init_device(device: &mut DeviceInfo) -> usize {
    // let mut device = DeviceInfo::new(fdt as *const u8).unwrap();
    let rw_region = get_sbi_region();
    log::info!(
        "sbi rw region: {:#x}..{:#x}",
        rw_region.start,
        rw_region.end
    );
    // let mut need_update = true;
    let mut target_region = None;
    log::info!("reserved memory region:");
    for mem in device.get_mem_region_reserved() {
        log::info!("region: {:?}..{:?}", mem.start, mem.end());
        if mem.end() as usize == rw_region.end {
            target_region = Some(mem);
            break;
        }
    }
    if let Some(region) = target_region {
        // let old_size = device
        //     .update_reserved_mem_region_size(region.start as usize, region.size + SM_RW_SIZE)
        //     .unwrap();
        // log::info!(
        //     "Update reserved memory area: starting address: {:#x} {old_size:#x} => {:#x}",
        //     region.start as usize,
        //     region.size + SM_RW_SIZE
        // );
        region.end() as usize
    } else {
        panic!()
    }
}

#[inline]
fn create_pma_mgr(mem_pool: &MemRegion) -> PhysMemAreaMgr {
    const NODE_SIZE: usize = PhysMemAreaMgr::NODE_SIZE;

    let mut mgr = PhysMemAreaMgr::new(unsafe {
        slice::from_raw_parts_mut(mem_pool.start as *mut u8, mem_pool.size)
    });

    mgr.insert_pma(PhysMemArea {
        region: 0..(usize::MAX - 0x1),
        prop: PmaProp::default()
            .owner(Owner::HOST)
            .permission(Permission::RWX),
    })
    .unwrap();

    mgr.insert_pma(PhysMemArea {
        region: (mem_pool.start as usize)..(mem_pool.start as usize + mem_pool.size),
        prop: PmaProp::empty(),
    })
    .unwrap();

    mgr
}

#[inline]
fn create_sm(mut device: DeviceInfo, rw_start: usize) {
    let pma_region = MemRegion {
        start: (rw_start + SM_HEAP_SIZE) as *const u8,
        size: (SM_RW_SIZE - SM_HEAP_SIZE),
    };
    let heap_region = MemRegion {
        start: rw_start as *const u8,
        size: SM_HEAP_SIZE,
    };

    log::debug!("pma memory: {pma_region}");
    log::debug!("heap memory: {heap_region}");

    let hart_num = count_hart_number();
    log::info!("hart num: {}", hart_num);
    device.hart_num = hart_num;

    unsafe {
        SM.assume_init_mut().init(&device);
        SM.assume_init_mut()
            .clint
            .init(device.get_clint_region().unwrap().start);
        SM.assume_init_mut()
            .init_heap(heap_region.start as *mut u8, heap_region.size);
        SM.assume_init_mut().init_pma(|| {
            // change the memory size
            let mut pm = create_pma_mgr(&pma_region);
            pm.insert_pma(PhysMemArea {
                region: FW_TEXT_START..rw_start,
                prop: PmaProp::empty()
                    .owner(Owner::EVERYONE)
                    .permission(Permission::NONE),
            })
            .unwrap();
            let uart_region = device.get_uart().unwrap().get_reg();
            pm.insert_pma(PhysMemArea {
                region: uart_region,
                prop: PmaProp::empty()
                    .permission(Permission::RW)
                    .owner(Owner::EVERYONE),
            })
            .unwrap();

            log::debug!("current pma:");
            for pma in pm.iter_pma() {
                log::debug!("{pma}")
            }
            pm
        });
    }
}

fn init_hart_trap(mtvec: usize) {
    unsafe { mtvec::write(mtvec, mtvec::TrapMode::Vectored) };
}

#[inline]
fn count_hart_number() -> usize {
    let mut hart_num = 0;
    for i in 0..MAX_HART_NUM {
        let rc = sbi_hsm_hart_get_state_ecall(i);
        if rc < 0 {
            break;
        }
        hart_num += 1;
    }
    hart_num
}
