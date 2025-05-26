#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(fn_align)]

// mod console;

use core::arch::naked_asm;

// use console::init_log;
use channel::enclave::runtime::LueBootArgs;
use rt::{
    consts::LUE_KERNEL_VADDR,
    kernel::LinuxUserKernel,
    log,
    pt::RtPtWriter,
    trap::{disable_any_interrupt, disable_supervisor_interrupt, lue_stvec_init},
    LuHeapAllocator, Mutex, PhysMemMgr,
};
use vm::{
    page_table::PTEFlags,
    vm::{Sv39VmMgr, VirtMemMgr, VirtPageNum},
};

/// Runtime entry
///
/// - mem_start: 整个内存空间的起始地址（虚拟地址）。
/// - mem_size: 整个内存空间的大小。
/// - bin_start: 二进制加载
#[naked]
#[no_mangle]
#[link_section = ".start"]
unsafe extern "C" fn _start(hartid: usize, args_addr: usize, satp: usize) -> ! {
    naked_asm!(
        "
            j {entry}
        ",
        entry = sym entry,
    )
}

//  mem_size: num of bytes
//  bin_size: num of bytes
#[no_mangle]
unsafe extern "C" fn entry(hartid: usize, args_addr: usize) -> ! {
    let args = (args_addr as *const LueBootArgs).read();

    let p_list = args.unmapped.head;
    let p_list_size = args.unmapped.size;
    let trampoline = args.tp.addr;
    let elf_start = args.bin.start;
    let elf_size = args.bin.size;
    let shared_args = args.shared;
    let ht_offset = shared_args.host_vaddr - shared_args.enc_vaddr;

    // core::arch::asm!("unimp");

    // init_log(0x10000000);

    // log::debug!("Debug mode");
    // panic!();

    // set trap vector
    lue_stvec_init();

    disable_supervisor_interrupt();

    // build frame allocator first
    let pmm = PhysMemMgr::new(p_list, p_list_size, trampoline);
    // alloca a frame
    let frame = pmm.get_free_frame().unwrap();

    // core::arch::asm!("unimp");
    // create the vmm
    let mut vmm = Sv39VmMgr::from_reg(
        RtPtWriter::new(pmm.clone_trampoline()),
        pmm.spawn_allocator(),
    );
    // map page for kernel
    vmm.map_frame(
        VirtPageNum::from_vaddr(LUE_KERNEL_VADDR),
        frame,
        PTEFlags::rw().accessed().dirty(),
    );

    // mutable kernel field
    {
        // build Kernel but uninit
        let kernel = LinuxUserKernel::uninit(LUE_KERNEL_VADDR);
        assert_eq!(kernel as *mut LinuxUserKernel as usize, LUE_KERNEL_VADDR);

        kernel.init_console(args.device.uart.clone());

        kernel.device = args.device;

        // init console

        // move pmm
        kernel.pmm = pmm;
        // as pmm moved, we need re-create vmm
        kernel.vmm = Mutex::new(VirtMemMgr::from_reg(
            RtPtWriter::new(kernel.pmm.clone_trampoline()),
            kernel.pmm.spawn_allocator(),
        ));

        // init scratch
        kernel.init_scratch(hartid);

        // init heap
        kernel.init_heap();

        // init shared memory
        let vstack_addr = kernel.init_vstack(shared_args.enc_vaddr, shared_args.size, hartid);

        kernel
            .create_task(elf_start, elf_size, vstack_addr, ht_offset)
            .unwrap();

        // recycle the old elf data
        log::debug!(
            "recycle the old elf data in {:#x}-{:#x}",
            elf_start,
            elf_start + elf_size
        );
        kernel.vmm.lock().dealloc_vma(elf_start, elf_size);
    }

    // // enable clock interrupt
    // enable_timer_interrupt();

    //disable clock interrupt
    // log::debug!("disable interrupt");
    disable_any_interrupt();

    // change the stack
    core::arch::asm!(
        // switch stack
        "csrrw  t0, sscratch, t0",
        "mv     sp, t0",
        "csrrw  t0, sscratch, t0",
    );

    // // recycle initial stack
    // kernel.recycle_pre_stack();

    let kernel = LinuxUserKernel::from_sscratch();

    // start task
    kernel.start_task();
}

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    log::error!("{}", _panic);
    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: LuHeapAllocator = LuHeapAllocator;
