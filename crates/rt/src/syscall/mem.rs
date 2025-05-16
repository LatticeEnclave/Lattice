use rand_core::le;
use sbi::tlb_flush;
use vm::{consts::PAGE_SIZE, page_table::PTEFlags, pm::PhysPageNum, vm::VirtPageNum};

use crate::kernel::{self, LinuxUserKernel};
use crate::log;

// mmap flags
const MAP_ANONYMOUS: usize = 0x20;
const MAP_PRIVATE: usize = 0x2;

// mmap prot
const PROT_READ: usize = 0x1; /* Page can be read.  */
const PROT_WRITE: usize = 0x2; /* Page can be written.  */
const PROT_EXEC: usize = 0x4; /* Page can be executed.  */
const PROT_NONE: usize = 0x0; /* Page can not be accessed.  */

pub fn sys_brk(addr: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let cur_break = kernel.task.get_break();

    // if addr is null return the current break
    if addr == 0 {
        return cur_break as isize;
    }

    if addr <= cur_break {
        return addr as isize;
    }

    // try to allocate pages
    let cur_vpn = VirtPageNum::from_vaddr(cur_break);
    let end_vpn = VirtPageNum::from_vaddr(addr - 1);

    log::debug!(
        "[sys_brk]: require page nums: {:#x} ",
        (end_vpn.0 - cur_vpn.0 + 1)
    );

    // check if exists enough space
    let free_size = kernel.pmm.get_spa_size();
    log::debug!("[sys_brk]: free space size: {:#x}", free_size);
    if free_size < (end_vpn.0 - cur_vpn.0 + 1) * PAGE_SIZE {
        log::debug!("[sys_brk]: dearth of free pages to brk expand, return error.");
        return -1;
    }

    let mut cnt = 0;

    for vpn in cur_vpn..=end_vpn {
        let ppn = kernel.task.vmm.lock().alloc_new_page(
            vpn,
            PTEFlags::W | PTEFlags::U | PTEFlags::R | PTEFlags::D | PTEFlags::A | PTEFlags::V,
        );
        if !ppn.is_none() {
            cnt += 1;
        }
    }

    log::debug!("[sys_brk]: actually allocated page nums: {:#x}", cnt);

    let new_break = (end_vpn.0 + 1) << 12;

    kernel.task.set_break(new_break);

    unsafe {
        tlb_flush!();
    }

    return addr as isize;
}

pub fn sys_mmap(
    addr: usize,
    len: usize,
    prot: usize,
    flag: usize,
    fd: isize,
    offset: u64,
) -> isize {
    if (flag != MAP_ANONYMOUS | MAP_PRIVATE) || (fd != -1) {
        return -1;
    }

    let mut pte_flags = PTEFlags::U | PTEFlags::A | PTEFlags::V;
    if (prot & PROT_READ) != 0 {
        pte_flags |= PTEFlags::R;
    }
    if (prot & PROT_WRITE) != 0 {
        pte_flags |= PTEFlags::W | PTEFlags::D;
    }
    if (prot & PROT_EXEC) != 0 {
        pte_flags |= PTEFlags::X;
    }

    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let cur_anon_start = kernel.task.get_anon_start();

    let cur_vpn = VirtPageNum::from_vaddr(cur_anon_start);
    let end_vpn = VirtPageNum::from_vaddr(cur_anon_start + len - 1);

    log::debug!(
        "[sys_mmap]: cur_vpn: {:#x}, end_vpn: {:#x}",
        cur_vpn.0,
        end_vpn.0
    );

    // check if exists enough space
    let free_size = kernel.pmm.get_spa_size();
    log::debug!("[sys_mmap]: free space size: {:#x}", free_size);
    if free_size < (end_vpn.0 - cur_vpn.0 + 1) * PAGE_SIZE {
        log::debug!("[sys_mmap]: dearth of free pages to mmap, return error.");
        return -1;
    }

    for vpn in cur_vpn..=end_vpn {
        let _ = kernel
            .task
            .vmm
            .lock()
            .alloc_new_page(vpn, pte_flags)
            .unwrap();
    }

    let new_anon_start = (end_vpn.0 + 1) << 12;

    kernel.task.set_anon_start(new_anon_start);

    unsafe {
        tlb_flush!();
    }

    return cur_anon_start as isize;
}

#[inline(never)]
pub fn sys_munmap(addr: usize, len: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let ret = kernel.delocate_user_vma(addr, len);

    unsafe {
        tlb_flush!();
    }

    if ret {
        return 0;
    }

    return -1;
}

pub fn sys_mprotect(addr: usize, len: usize, prot: usize) -> isize {
    let mut pte_flags = PTEFlags::U | PTEFlags::V | PTEFlags::A | PTEFlags::D;
    if (prot & PROT_READ) != 0 {
        pte_flags |= PTEFlags::R;
    }
    if (prot & PROT_WRITE) != 0 {
        pte_flags |= PTEFlags::W;
    }
    if (prot & PROT_EXEC) != 0 {
        pte_flags |= PTEFlags::X;
    }

    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let ret = kernel.task.vmm.lock().remap_vma(addr, len, pte_flags);

    unsafe {
        tlb_flush!();
    }

    if ret {
        return 0;
    }

    return -1;
}
