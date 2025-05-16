use crate::log;
use alloc::boxed::Box;
use alloc::collections::btree_map::Entry;
use alloc::vec;
use alloc::vec::Vec;
use htee_device::device::Device;
use core::marker::PhantomPinned;
use core::mem::size_of;
use core::ops::DerefMut;
use core::ptr::NonNull;
use core::sync::atomic::AtomicUsize;
use data_structure::linked_list;
use elf::Sections;
use elf_loader::ElfObject;
use htee_device::console::Uart;
use htee_vstack::Vstack;
use load_module::{load_driver_by_sections, load_section_at, load_text_at, SymbolTable};
use riscv::register::mstatus::set_spp;
use riscv::register::{sie, sstatus};
use spin::Mutex;
use uart::MmioUart;
use vm::mm::SV39;
use vm::page_table::PTEFlags;
use vm::vm::{Sv39VmMgr, VirtMemMgr, VirtPageNum};
use xmas_elf::ElfFile;

use crate::console::Console;
use crate::ldesym::init_symbol_table;
use crate::{
    consts::*,
    frame::{PhysMemMgr, RtFrameAlloc},
    heap::Heap,
    loader::RuntimeElfLoader,
    pt::RtPtWriter,
    runtime_fault,
    scratch::switch_scratch,
    scratch::ScratchManager,
    stack::StackEnv,
    syscall::RandGenerator,
    task::Task,
    trap::trap_to_process,
    Result, Scratch,
};

#[derive(Debug, Clone, Copy)]
pub enum KernelPtr {
    LUKPtr(NonNull<LinuxUserKernel>),
    LDKPtr(NonNull<LinuxDriverKernel>),
}

/// kernel object
#[repr(C)]
pub struct LinuxUserKernel {
    pid_counter: AtomicUsize,
    pub pmm: PhysMemMgr,
    pub vmm: Mutex<Sv39VmMgr<RtPtWriter, RtFrameAlloc>>, // vmm for runtime virtual space
    pub heap: Heap,
    pub scratch_manager: &'static mut ScratchManager,
    pub task: Task,
    elocked_ppns: Mutex<Vec<usize>>,
    pub console: Console,
    pub device: Device,
    __pinned: PhantomPinned,
}

impl LinuxUserKernel {
    pub fn uninit(addr: usize) -> &'static mut Self {
        unsafe { &mut *(addr as *mut Self) }
    }

    /// Notes: user must ensure the sscratch register is right.
    pub unsafe fn from_sscratch() -> &'static Self {
        &Scratch::from_ssratch().get_lu_kernel()
    }

    /// 初始化堆
    pub fn init_heap(&mut self) {
        let heap_end_vpn = VirtPageNum::from_vaddr(LUE_HEAP_START + HEAP_SIZE - 1);
        let heap_start_vpn = VirtPageNum::from_vaddr(LUE_HEAP_START);

        for vpn in heap_start_vpn..=heap_end_vpn {
            // for vpn in heap_start_vpn..(heap_start_vpn.add())
            let ppn = self
                .pmm
                .get_free_frame()
                .expect("failed to allocate physical page for Heap");
            self.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }

        // let heap = Heap
        unsafe {
            self.heap = Heap::new(LUE_HEAP_START, HEAP_SIZE);
        }
    }

    pub fn init_console(&mut self, uart: Uart) {
        let console =
            Console::new(MmioUart::new(uart.addr, uart.reg_shift, uart.reg_io_width));
        self.console = console;
    }

    pub fn get_console(&self) -> &Console {
        &self.console
    }

    /// initiat shared memory region
    pub fn init_vstack(&mut self, shared_start: usize, shared_size: usize, hartid: usize) -> usize {
        let vstack_size = shared_size / MAX_HART_NUM;
        let vstack_addr = shared_start + vstack_size * hartid;
        Vstack::new(vstack_addr, vstack_size);

        vstack_addr
    }

    pub fn init_scratch(&mut self, hartid: usize) {
        // 获取 ScratchManager 的大小
        let size = size_of::<ScratchManager>();
        // 计算所需的页面数
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        // 将预定的虚拟地址转换为虚拟页号
        let vpn_start = VirtPageNum::from_vaddr(LUE_SCRATCH_START_VADDR);

        // 逐个分配物理页面并将其映射到虚拟地址空间
        for vpn in vpn_start..vpn_start.add(num_pages) {
            let ppn = self
                .pmm
                .get_free_frame()
                .expect("failed to allocate physical page for ScratchManager");
            self.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }

        // 在映射的虚拟地址上初始化 ScratchManager
        let mgr = unsafe { ScratchManager::uninit_at(LUE_SCRATCH_START_VADDR) };
        let kernel_ref: &LinuxUserKernel = &self;
        mgr.init_all(KernelPtr::LUKPtr(NonNull::from(kernel_ref)));
        self.scratch_manager = mgr;
        switch_scratch(&self.scratch_manager, hartid);

        let satp = self.vmm.get_mut().gen_satp();
        // set sscratch pre_satp as task satp
        unsafe {
            core::arch::asm!(
            "csrrw  tp, sscratch, tp",
            "sd     {}, {}(tp)",
            "csrrw  tp, sscratch, tp",
            in(reg) satp,
            const core::mem::offset_of!(Scratch, prev_satp),
            );
        }
    }

    pub fn get_heap(&self) -> &Heap {
        &self.heap
    }

    pub fn fetch_pid(&self) -> usize {
        let pid = self
            .pid_counter
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        pid & 0xffff
    }

    pub fn start_task(&self) -> ! {
        log::debug!("starting task");
        let mut ustatus: usize = 0;

        unsafe {
            core::arch::asm!(
                "csrr     {ustatus}, sstatus",
                ustatus = out(reg) ustatus,
            )
        }

        ustatus = ustatus & 0xffffffffffffff7f;
        ustatus = ustatus | 0x0000000000040000;

        let entry = self.task.get_entry();
        let sp = self.task.get_sp();
        log::debug!("start task, entry: {:#x}, sp: {:#x}", entry, sp);
        // test entering the process, no argc and argv, sstatus not handled yet
        let task_lock = self.task.vmm.lock();
        let satp = task_lock.gen_satp();
        log::debug!("task satp: {:#x}", satp);
        core::mem::drop(task_lock);
        trap_to_process(0, 0, entry, sp, satp, ustatus)
    }

    pub fn create_task(
        &mut self,
        bin_start: usize,
        bin_size: usize,
        vstack_addr: usize,
        ht_offset: usize,
    ) -> Result<usize> {
        // create task
        let pid = self.fetch_pid();
        // create vmm for task
        // alloc a free page for task root page table
        let task_root = self.pmm.get_free_frame().ok_or(runtime_fault!())?;
        let task_rt_writer = RtPtWriter::new(self.pmm.clone_trampoline());
        let task_frame_allocator = self.pmm.spawn_allocator();
        let task_vmm = VirtMemMgr::new(task_root, task_rt_writer, task_frame_allocator, pid, SV39);
        // new task, use given pid and vmm
        let mut task = Task::new(pid, ht_offset, task_vmm, vstack_addr);

        log::debug!("loading elf");
        // load elf and set task
        let (entry, phdr, phnum) = self.load_elf(&mut task, bin_start, bin_size);

        log::debug!("set entry to {:#x}", entry);
        //set task
        task.set_entry(entry);

        log::debug!("alloc stack for task, phdr: {:#x}, phnum: {}", phdr, phnum);
        // alloc stack
        self.alloc_stack(&mut task, phdr, phnum);

        log::debug!("mapping sscratch for task");
        // map ssractch in task virtual space
        self.map_sscratch(&mut task);

        log::debug!("mapping trap entry for task");
        // task trampoline
        self.map_trap_entry(&mut task);

        self.task = task;
        log::debug!("task created");

        Ok(entry)
    }

    fn load_elf(
        &self,
        task: &mut Task,
        bin_start: usize,
        bin_size: usize,
    ) -> (usize, usize, usize) {
        // start load elf
        // we need lock kernel space vmm first
        let mut vmm_lock = self.vmm.lock();
        let mut loader = RuntimeElfLoader::new(&mut vmm_lock);
        // get the elf file raw data
        let elf_data = unsafe { core::slice::from_raw_parts(bin_start as *mut u8, bin_size) };
        let elf_object = ElfObject::new(elf_data).unwrap();

        let phnum = elf_object.get_phnum();
        let phdr = elf_object.get_phdr();

        log::debug!("[usr env]: phnum: {:#x}, phdr: {:#x}", phnum, phdr);

        log::debug!(
            "loading elf. elf data placed on {:#x}",
            elf_data.as_ptr() as usize
        );
        // laod elf and get the entry
        let entry = loader.load_elf(task, elf_object).unwrap();

        (entry, phdr, phnum)
    }

    fn alloc_stack(&self, task: &mut Task, phdr: usize, phnum: usize) {
        // calculate the number of pages needed for the stack
        let num_pages = (STACK_SIZE + PAGE_SIZE - 1) / PAGE_SIZE;

        // allocate physical pages for the stack and map them to the task's virtual address space
        let vpn_base = VirtPageNum::from_vaddr(TASK_STACK_TOP - STACK_SIZE);
        for i in 0..num_pages {
            // allocate a physical page for each virtual page
            let ppn = self.pmm.get_free_frame().unwrap();
            let vpn = vpn_base.add(i);
            task.vmm.lock().map_frame(vpn, ppn, PTEFlags::urwx().accessed().dirty());
        }

        self.setup_usr_env(task, phdr, phnum);
    }

    fn setup_usr_env(&self, task: &mut Task, phdr: usize, phnum: usize) {
        const AT_NULL: usize = 0;
        const AT_PHDR: usize = 3;
        const AT_PHNUM: usize = 5;
        const AT_PAGESZ: usize = 6;
        const AT_UID: usize = 11;
        const AT_EUID: usize = 12;
        const AT_GID: usize = 13;
        const AT_EGID: usize = 14;
        const AT_HWCAP: usize = 16;
        const AT_SECURE: usize = 23;
        const AT_RANDOM: usize = 25;
        const AT_EXECFN: usize = 31;
        // const AT_SYSINFO: usize = 32;

        let sp = TASK_STACK_TOP - core::mem::size_of::<StackEnv>();
        let mut rand = RandGenerator::new(sp);
        let r1 = rand.next_u64();
        let r2 = rand.next_u64();

        let usr_env = StackEnv {
            argc: 0,
            argv: 0,
            envp: 0,
            hwcap_key: AT_HWCAP,
            hwcap_val: 0x112d,
            // sysinfo_key: AT_SYSINFO,
            // sysinfo_val: 0x0,
            pagesz_key: AT_PAGESZ,
            pagesz_val: 0x1000,
            execfn_key: AT_EXECFN,
            execfn_val: 0,
            sec_key: AT_SECURE,
            sec_val: 0,
            rand_key: AT_RANDOM,
            rand_val: TASK_STACK_TOP - 2 * core::mem::size_of::<usize>(),
            gid_key: AT_GID,
            gid_val: 1,
            egid_key: AT_EGID,
            egid_val: 1,
            uid_key: AT_UID,
            uid_val: 1,
            euid_key: AT_EUID,
            euid_val: 1,
            phdr_key: AT_PHDR,
            phdr_val: phdr,
            phnum_key: AT_PHNUM,
            phnum_val: phnum,
            null_key: AT_NULL,
            null_val: 0,
            rand_num1: r1 as usize,
            rand_num2: r2 as usize,
        };

        // write env to the usr stack space
        let usr_sp_vpn = VirtPageNum::from_vaddr(sp);
        let usr_sp_ppn = task.vmm.lock().get_pte(usr_sp_vpn).unwrap().get_ppn();

        {
            self.vmm
                .lock()
                .map_frame(usr_sp_vpn, usr_sp_ppn, PTEFlags::rwx().accessed().dirty());
        }

        unsafe {
            (sp as *mut StackEnv).write(usr_env);
        }

        self.vmm.lock().unmap_frame(usr_sp_vpn);

        task.set_sp(sp);
    }

    fn map_sscratch(&self, task: &mut Task) {
        // 获取 ScratchManager 的大小
        let size = size_of::<ScratchManager>();
        // 计算所需的页面数
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        // 将预定的虚拟地址转换为虚拟页号
        let vpn_start = VirtPageNum::from_vaddr(LUE_SCRATCH_START_VADDR);

        // 逐个分配物理页面并将其映射到虚拟地址空间
        for vpn in vpn_start..vpn_start.add(num_pages) {
            let ppn = self.vmm.lock().get_pte(vpn).unwrap().get_ppn();
            task.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }
    }

    pub fn map_trap_entry(&self, task: &mut Task) {
        // trap entry in virtual space
        let vpn = VirtPageNum::from_vaddr(RUNTIME_VA_START);
        // trap physical page num
        let ppn = self.vmm.lock().get_pte(vpn).unwrap().get_ppn();

        task.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
    }

    pub fn delocate_user_vma(&self, addr: usize, size: usize) -> bool {
        for vaddr in (addr..(addr + size)).step_by(PAGE_SIZE) {
            let ppn = self.task.vmm.lock().get_pte(VirtPageNum::from_vaddr(vaddr));
            if ppn.is_none() {
                return false;
            }

            {
                self.vmm.lock().map_frame(
                    VirtPageNum::from_vaddr(USER_CLEAN_BUFFER),
                    ppn.unwrap().get_ppn(),
                    PTEFlags::rw().accessed().dirty(),
                );
            }

            // clear the page
            unsafe {
                core::ptr::write_bytes(USER_CLEAN_BUFFER as *mut u8, 0, PAGE_SIZE);
            }

            {
                self.vmm
                    .lock()
                    .unmap_frame(VirtPageNum::from_vaddr(USER_CLEAN_BUFFER));
            }
            {
                self.task
                    .vmm
                    .lock()
                    .dealloc_frame(VirtPageNum::from_vaddr(vaddr));
            }
        }
        true
    }
}

/// kernel object
#[repr(C)]
pub struct LinuxDriverKernel {
    pid_counter: AtomicUsize,
    pub pmm: PhysMemMgr,
    pub vmm: Mutex<Sv39VmMgr<RtPtWriter, RtFrameAlloc>>, // vmm for runtime virtual space
    pub heap: Heap,
    pub scratch_manager: &'static mut ScratchManager,
    // channel: Mutex<usize>,
    pub stack_pages: Mutex<Vec<usize>>,
    __pinned: PhantomPinned,
}

impl LinuxDriverKernel {
    pub fn uninit(addr: usize) -> &'static mut Self {
        unsafe { &mut *(addr as *mut Self) }
    }

    /// Notes: user must ensure the sscratch register is right.
    pub unsafe fn from_sscratch() -> &'static Self {
        &Scratch::from_ssratch().get_ld_kernel()
    }

    /// 初始化堆
    pub fn init_heap(&mut self) {
        let heap_end_vpn = VirtPageNum::from_vaddr(LDE_HEAP_START + HEAP_SIZE - 1);
        let heap_start_vpn = VirtPageNum::from_vaddr(LDE_HEAP_START);

        for vpn in heap_start_vpn..=heap_end_vpn {
            // for vpn in heap_start_vpn..(heap_start_vpn.add())
            let ppn = self
                .pmm
                .get_free_frame()
                .expect("failed to allocate physical page for Heap");
            self.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }

        // let heap = Heap
        unsafe {
            self.heap = Heap::new(LDE_HEAP_START, HEAP_SIZE);
        }
    }

    /// init elocked page vec
    pub fn init_stack_pages(&mut self) {
        self.stack_pages = Mutex::new(Vec::new());
    }

    pub fn init_scratch(&mut self, hartid: usize) {
        // 获取 ScratchManager 的大小
        let size = size_of::<ScratchManager>();
        // 计算所需的页面数
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        // 将预定的虚拟地址转换为虚拟页号
        let vpn_start = VirtPageNum::from_vaddr(LDE_SCRATCH_START_VADDR);

        // 逐个分配物理页面并将其映射到虚拟地址空间
        for vpn in vpn_start..vpn_start.add(num_pages) {
            let ppn = self
                .pmm
                .get_free_frame()
                .expect("failed to allocate physical page for ScratchManager");
            self.vmm.lock().map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }

        // 在映射的虚拟地址上初始化 ScratchManager
        let mgr = unsafe { ScratchManager::uninit_at(LDE_SCRATCH_START_VADDR) };
        let kernel_ref: &LinuxDriverKernel = &self;
        mgr.init_all(KernelPtr::LDKPtr(NonNull::from(kernel_ref)));
        self.scratch_manager = mgr;
        switch_scratch(&self.scratch_manager, hartid);

        let satp = self.vmm.get_mut().gen_satp();
        // set sscratch pre_satp as task satp
        unsafe {
            core::arch::asm!(
            "csrrw  tp, sscratch, tp",
            "sd     {}, {}(tp)",
            "csrrw  tp, sscratch, tp",
            in(reg) satp,
            const core::mem::offset_of!(Scratch, prev_satp),
            );
        }
    }

    pub fn get_heap(&self) -> &Heap {
        &self.heap
    }

    pub fn fetch_pid(&self) -> usize {
        let pid = self
            .pid_counter
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        pid & 0xffff
    }

    pub fn load_driver(
        &mut self,
        bin_start: usize,
        bin_size: usize,
        driver_start: usize,
        driver_size: usize,
        sections: Sections,
    ) -> Result<()> {
        // the global symbol table
        let mut symtab = SymbolTable::new();
        init_symbol_table(&mut symtab);

        // map physical pages for the driver
        self.map_driver_region(driver_start, driver_size);

        // get the elf file raw data
        let driver_data = unsafe { core::slice::from_raw_parts(bin_start as *mut u8, bin_size) };

        // // 解析ELF文件
        // let elf_file = match ElfFile::new(driver_data) {
        //     Ok(file) => file,
        //     Err(_) => return Err(crate::Error::KernelInitErr("Failed to parse ELF file")),
        // };

        // relocate buffer
        let data = unsafe { core::slice::from_raw_parts_mut(driver_start as *mut u8, driver_size) };

        if let Err(e) = load_driver_by_sections(driver_data, data, driver_start, &symtab, sections)
        {
            log::error!("{}", e);
        }

        // if sections.text != 0 {
        //     log::debug!("load .text section");
        //     let data = &mut data[(sections.text - driver_start)..];
        //     if let Err(e) = load_section_at(
        //         &elf_file,
        //         data,
        //         sections.text,
        //         &symtab,
        //         ".text",
        //         ".rela.text",
        //     ) {
        //         log::error!("{}", e);
        //     }
        // }

        // if sections.text_unlikely != 0 {
        //     log::debug!("load .text.unlikely section");
        //     let data = &mut data[(sections.text_unlikely - driver_start)..];
        //     if let Err(e) = load_section_at(
        //         &elf_file,
        //         data,
        //         sections.text_unlikely,
        //         &symtab,
        //         ".text.unlikely",
        //         ".rela.text.unlikely",
        //     ) {
        //         log::error!("{}", e);
        //     }
        // }

        Ok(())
    }

    fn map_driver_region(&self, driver_start: usize, driver_size: usize) {
        // calculate the number of pages needed for the driver
        let num_pages = (driver_size + PAGE_SIZE - 1) / PAGE_SIZE;

        // allocate physical pages for the driver and map them to the kernel space
        let vpn_base = VirtPageNum::from_vaddr(driver_start);
        let mut lock = self.vmm.lock();
        for i in 0..num_pages {
            // allocate a physical page for each virtual page
            let ppn = self.pmm.get_free_frame().unwrap();
            let vpn = vpn_base.add(i);
            lock.map_frame(vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }
    }

    // pub fn set_channel(&self, channel_id: usize) {
    //     let mut lock = self.channel.lock();
    //     *lock.deref_mut() = channel_id.into();
    // }

    // pub fn get_chanel(&self) -> usize {
    //     let lock = self.channel.lock();
    //     lock.clone()
    // }
}
