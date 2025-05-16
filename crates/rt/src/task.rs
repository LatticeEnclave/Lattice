use core::{mem::offset_of, ops::DerefMut, ptr::NonNull, slice::SliceIndex};
use alloc::collections::btree_map::Entry;
use data_structure::linked_list::{self, Node};
use htee_vstack::Vstack;
use spin::mutex::Mutex;
use vm::vm::{Sv39VmMgr, VirtMemMgr};

use crate::{consts::USR_ANON_REGION_START, frame::RtFrameAlloc, kernel::LinuxUserKernel, pt::RtPtWriter, stack::Stack};

#[repr(C)]
pub struct Task {
    pid: usize,
    ht_offset: usize,
    entry: Option<usize>,
    sp: Option<usize>,
    program_break: Mutex<usize>,
    mmap_anon_region: Mutex<usize>,
    vstack: NonNull<Vstack>,
    pub vmm: Mutex<Sv39VmMgr<RtPtWriter, RtFrameAlloc>>,
    // page_table root for task virtual space
}

impl Task {
    pub fn new(pid: usize, ht_offset: usize, vmm: Sv39VmMgr<RtPtWriter, RtFrameAlloc>, vstack: usize) -> Self {
        Self {
            pid: pid,
            ht_offset: ht_offset,
            entry: None,
            sp: None,
            program_break: Mutex::new(USR_ANON_REGION_START + 1024 * 1024 * 1024),
            mmap_anon_region: Mutex::new(USR_ANON_REGION_START),
            vstack: NonNull::new(vstack as *mut Vstack).unwrap(),
            vmm: Mutex::new(vmm),
        }
    }

    pub fn get_pid(&self) -> usize {
        self.pid
    }

    pub fn get_break(&self) -> usize {
        let lock = self.program_break.lock();
        lock.clone()
    }

    pub fn get_entry(&self) -> usize {
        self.entry.unwrap()
    }

    pub fn get_sp(&self) -> usize {
        self.sp.unwrap()
    }

    pub fn get_vstack_addr(&self) -> usize {
        self.vstack.as_ptr() as usize
    }

    pub fn get_ht_offset(&self) -> usize {
        self.ht_offset
    }

    pub fn set_break(&self, new_break: usize) {
        let mut lock = self.program_break.lock();
        *lock.deref_mut() = new_break.into();
    }

    pub fn get_anon_start(&self) -> usize {
        let lock = self.mmap_anon_region.lock();
        lock.clone()
    }

    pub fn set_anon_start(&self, new_start: usize) {
        let mut lock = self.mmap_anon_region.lock();
        *lock.deref_mut() = new_start.into();        
    }

    pub fn set_entry(&mut self, entry: usize) {
        self.entry = Some(entry);
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.sp = Some(sp);
    }
}
