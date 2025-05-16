#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(offset_of)]
#![feature(fn_align)]

// mod console;

use core::arch::{asm, naked_asm};

use htee_channel::enclave::runtime::LdeBootArgs;
use htee_console::{init_console_uart, log};
use rt::{
    LdHeapAllocator, Mutex, PhysMemMgr,
    consts::{LDE_KERNEL_VADDR, PAGE_SIZE, RUNTIME_VA_START},
    kernel::LinuxDriverKernel,
    pt::RtPtWriter,
    syscall::sbi_stop_enclave,
    trap::{disable_timer_interrupt, enable_timer_interrupt, lde_stvec_init},
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
#[unsafe(no_mangle)]
#[unsafe(link_section = ".start")]
unsafe extern "C" fn _start(hartid: usize, args_addr: usize) -> ! {
    unsafe {
        naked_asm!(
            "
                j {entry}
            ",
            entry = sym entry,
        )
    }
}

//  mem_size: num of bytes
//  bin_size: num of bytes
#[unsafe(no_mangle)]
unsafe extern "C" fn entry(hartid: usize, args_addr: usize) {
    let args = unsafe { (args_addr as *const LdeBootArgs).read() };

    let p_list = args.unmapped.head;
    let p_list_size = args.unmapped.size;
    let trampoline = args.tp.addr;
    let elf_start = args.bin.start;
    let elf_size = args.bin.size;
    let driver_start = args.driver_start;
    let driver_size = args.driver_size;
    let device = args.device;

    init_console_uart(device.uart);
    log::info!("init console uart");

    lde_stvec_init();

    // build frame allocator first
    let pmm = PhysMemMgr::new(p_list, p_list_size, trampoline);
    // alloca a frame
    let frame = pmm.get_free_frame().unwrap();
    // create the vmm
    let mut vmm = Sv39VmMgr::from_reg(
        RtPtWriter::new(pmm.clone_trampoline()),
        pmm.spawn_allocator(),
    );
    // map page for kernel
    vmm.map_frame(
        VirtPageNum::from_vaddr(LDE_KERNEL_VADDR),
        frame,
        PTEFlags::rwx(),
    );

    // mutable kernel field
    {
        // build Kernel but uninit
        let kernel = LinuxDriverKernel::uninit(LDE_KERNEL_VADDR);
        assert_eq!(kernel as *mut LinuxDriverKernel as usize, LDE_KERNEL_VADDR);

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

        // init lde elocked pages vec
        kernel.init_stack_pages();

        // initiate the driver inside enclave
        kernel.load_driver(
            elf_start,
            elf_size,
            driver_start,
            driver_size,
            args.sections,
        );

        // recycle the old elf data
        kernel.vmm.lock().dealloc_vma(elf_start, elf_size);
    }

    // // enable clock interrupt
    // enable_timer_interrupt();

    //disable clock interrupt
    disable_timer_interrupt();

    // change the stack
    unsafe {
        core::arch::asm!(
            // switch stack
            "csrrw  t0, sscratch, t0",
            "mv     sp, t0",
            "csrrw  t0, sscratch, t0",
        );
    }

    // // recycle initial stack
    // kernel.recycle_pre_stack();

    // lde suspend
    sbi_stop_enclave(0);
}

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    log::error!("{}", _panic);
    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: LdHeapAllocator = LdHeapAllocator;
