use core::{
    arch::asm,
    slice,
    sync::atomic::{AtomicBool, Ordering},
};

use clint::ClintClient;
use hsm::MAX_HART_NUM;
use htee_console::{init_console_uart, log};
use htee_device::device::{Device, DeviceInfo};
use pma::{Owner, PhysMemArea, PhysMemAreaMgr, PmaProp};
use pmp::PmpStatus;
use riscv::register::{Permission, mepc, mhartid, mstatus, mtvec, stvec};
use spin::RwLock;
use trap_proxy::TrapProxy;
use vm::aligned;

use crate::{Error, Platform, SecMonitor, enclave::EnclaveMgr, trap::TrapHandler};

pub unsafe fn init<P: Platform>(platform: &P, next_addr: usize, arg1: usize) -> ! {
    static IS_COLD: AtomicBool = AtomicBool::new(true);
    // cold boot hart, arg0 = hartid, arg1 = fdt
    if IS_COLD.swap(false, Ordering::AcqRel) {
        unsafe { boothart_init(platform, arg1) };
    }

    unsafe { common_init(platform) }
    unsafe {
        stvec::write(next_addr, stvec::TrapMode::Direct);
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        mepc::write(next_addr);

        asm!(
            "mret",
            in("a0") mhartid::read(),
            in("a1") arg1,
            options(noreturn)
        )
    }
}

#[inline(never)]
unsafe fn boothart_init<P: Platform>(platform: &P, fdt: usize) {
    #[allow(static_mut_refs)]
    let sm = unsafe { crate::SM.assume_init_mut() };

    let device = DeviceInfo::new(fdt as *const u8).unwrap();

    init_console_uart(device.get_uart().unwrap());

    log::debug!("fdt: {fdt:#x}");
    log::debug!("PMP COUNT: {}", platform.get_pmp_count());
    log::debug!("{} do init", mhartid::read());

    // #[cfg(debug_assertions)]
    // for node in device_info.iter_all_nodes() {
    //     log::debug!("node: {}", node.name);
    // }

    // if let Some(dma) = device_info.get_dma() {
    //     log::debug!("dma: {:#x}", dma as usize);
    // }

    // init hsm
    init_hsm(sm, platform);
    log::debug!("Inited hsm. Hart num: {}", sm.hsm.num());

    init_clint(sm, &device).unwrap();
    log::debug!("Inited clint");

    // init pmp buffers
    init_buffer(sm, platform);
    log::debug!("Inited pmp buffers");

    // let pma_region = platform.get_pma_region();
    init_pma(sm, platform);
    log::debug!("Inited pma");

    init_enclave(sm);
    log::debug!("Inited enclave");

    init_device(sm, &device);
    log::debug!("Inited device");

    // initialize the memory region, and update the reserved memory area
    // create_sm(device, rw_start);

    // update sbi trap handler
    let sbi_handler = mtvec::read().address();
    unsafe { TrapHandler::init_redirect(sbi_handler) };
}

unsafe fn common_init<P: Platform>(_: &P) {
    let hartid = mhartid::read();
    log::debug!("{} start", hartid);

    // update hart pmp, clean all permission
    init_hart_pmp(pmp::PMP_COUNT);
    log::debug!("inited hart {hartid} pmp");

    unsafe { mtvec::write(TrapHandler::proxy as usize, mtvec::TrapMode::Direct) };
}

#[inline]
fn init_hart_pmp(pmp_count: usize) {
    for i in 0..pmp_count {
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
fn init_buffer<P: Platform>(sm: &mut SecMonitor, platform: &P) {
    use core::ptr::NonNull;
    use heapless::Vec;
    use pmp::PmpBuf;

    type BufPool = Vec<PmpBuf, MAX_HART_NUM>;

    let heap_region = platform.get_heap_region();
    log::debug!("heap region: {:#x?}", heap_region);

    assert!(aligned!(heap_region.start, 0x8));
    assert!(core::mem::size_of::<BufPool>() <= heap_region.len());
    let mut ptr = NonNull::new(heap_region.start as *mut BufPool).unwrap();
    unsafe {
        *ptr.as_mut() = Vec::new();
        for (i, hs) in sm.hsm.iter_hs_mut().enumerate() {
            ptr.as_mut().push(Vec::new()).unwrap();
            hs.pmp_buf = NonNull::new(ptr.as_mut().get_mut(i).unwrap()).unwrap()
        }
    }
}

#[inline]
fn init_pma<P: Platform>(sm: &mut SecMonitor, platform: &P) {
    let pma_region = platform.get_pma_region();
    log::debug!("pma region: {:#x?}", pma_region);

    let mut mgr = PhysMemAreaMgr::new(unsafe {
        slice::from_raw_parts_mut(pma_region.start as *mut u8, pma_region.len())
    });

    mgr.insert_pma(PhysMemArea {
        region: 0..(usize::MAX - 0x1),
        prop: PmaProp::default()
            .owner(Owner::HOST)
            .permission(Permission::RWX),
    })
    .unwrap();

    let region = platform.get_sbi_region();
    mgr.insert_pma(PhysMemArea {
        region,
        prop: PmaProp::empty(),
    })
    .unwrap();

    let region = platform.get_heap_region();
    mgr.insert_pma(PhysMemArea {
        region,
        prop: PmaProp::empty(),
    })
    .unwrap();

    let region = platform.get_pma_region();
    mgr.insert_pma(PhysMemArea {
        region,
        prop: PmaProp::empty(),
    })
    .unwrap();

    sm.pma_mgr = RwLock::new(mgr);
}

fn init_hsm<P: Platform>(sm: &mut SecMonitor, platform: &P) {
    let hsm = hsm::Hsm::new(platform.get_hart_num());
    sm.hsm = hsm;
}

fn init_enclave(sm: &mut SecMonitor) {
    sm.enc_mgr = EnclaveMgr::new();
}

fn init_clint(sm: &mut SecMonitor, device: &DeviceInfo) -> Result<(), Error> {
    sm.clint = ClintClient::new(sm.hsm.num());
    sm.clint.init(
        device
            .get_clint_region()
            .ok_or(Error::Other("clint not found"))?
            .start,
    );

    Ok(())
}

fn init_device(sm: &mut SecMonitor, device: &DeviceInfo) {
    sm.device = Device::from_device_info(device).unwrap();
}
