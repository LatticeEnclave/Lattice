use console::log;
use core::{cell::RefCell, fmt::Display};
use enclave::{
    EnclaveId, EnclaveIdGenerator, LinuxServiceEnclave, LinuxServiceEnclaveList, LinuxUserEnclave,
    LinuxUserEnclaveList,
};
use riscv::register::satp;
use spin::Mutex;
use vm::{
    allocator::FrameAllocator,
    page_table::{BarePtWriter, PTEFlags},
    prelude::*,
    trans_direct,
    vm::Sv39VmMgr,
    PAGE_SIZE,
};

pub const EXT_ID: usize = sbi::ecall::SBI_EXT_TEE_ENCLAVE;
pub const CREATE_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMCreateEnclave as usize;
pub const DESTROY_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMDestroyEnclave as usize;
pub const LAUNCH_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMRunEnclave as usize;
pub const RESUME_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMResumeEnclave as usize;
pub const PAUSE_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMStopEnclave as usize;
pub const EXIT_ENC: usize = sbi::ecall::SBISMEnclaveCall::SbiSMExitEnclave as usize;

#[derive(Default)]
pub struct UserArgs {
    pub mem: VirtMemArea,
    pub rt: VirtMemArea,
    pub binary: VirtMemArea,
    pub share: VirtMemArea,
    pub unused: VirtMemArea,
}

impl Display for UserArgs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "memory:  {}
runtime: {}
binary:  {}
share:   {}
unused:  {}",
            self.mem, self.rt, self.binary, self.share, self.unused
        ))
    }
}

pub struct EnclaveMgr {
    eid_gen: EnclaveIdGenerator,
    lue_list: Mutex<LinuxUserEnclaveList>,
    lse_list: Mutex<LinuxServiceEnclaveList>,
}

impl EnclaveMgr {
    pub fn new() -> Self {
        Self {
            eid_gen: EnclaveIdGenerator::new(),
            lue_list: Mutex::new(LinuxUserEnclaveList::new()),
            lse_list: Mutex::new(LinuxServiceEnclaveList::new()),
        }
    }

    pub fn get_lse(&self, _: impl Into<EnclaveId>) -> Option<&'static mut LinuxServiceEnclave> {
        self.lse_list.lock().first()
    }

    pub fn get_lue(&self, id: impl Into<EnclaveId>) -> Option<&'static mut LinuxUserEnclave> {
        self.lue_list.lock().get(id.into())
    }

    pub fn get_new_eid(&self) -> EnclaveId {
        self.eid_gen.fetch()
    }

    pub fn push_lue(&self, lue: &'static mut LinuxUserEnclave) {
        self.lue_list.lock().push(lue);
    }

    pub fn push_lse(&self, lse: &'static mut LinuxServiceEnclave) {
        self.lse_list.lock().push(lse);
    }

    pub fn rm_lue(&self, eid: EnclaveId) -> Option<&'static mut LinuxUserEnclave> {
        self.lue_list.lock().remove(eid)
    }
}

pub struct Builder {
    pub vmm: Sv39VmMgr<BarePtWriter, BuilderAllocator>,
}

impl Builder {
    pub fn create_lue(
        &mut self,
        userargs: &UserArgs,
        eid: EnclaveId,
    ) -> &'static mut LinuxUserEnclave {
        // create enclave at first page
        let meta_page = VirtAddr(userargs.mem.start)
            .translate(
                userargs.mem.satp.ppn(),
                userargs.mem.satp.mode(),
                &BarePtReader,
            )
            .unwrap();
        log::debug!("meta page: {:#x}", meta_page.0);
        enclave::create_lue_at(meta_page.0, eid)
    }

    pub fn create_lse(
        &mut self,
        userargs: &UserArgs,
        eid: EnclaveId,
    ) -> &'static mut LinuxServiceEnclave {
        // alloc meta page and update meta page ownership
        let meta_page = VirtAddr(userargs.mem.start)
            .translate(
                userargs.mem.satp.ppn(),
                userargs.mem.satp.mode(),
                &BarePtReader,
            )
            .unwrap();
        log::debug!("meta page: {:#x}", meta_page.0);
        enclave::create_lse_at(meta_page.0, eid)
    }

    pub fn create_trampoline(&mut self, vma: VirtMemArea) -> VirtMemArea {
        let mut tp = VirtMemArea::default().satp(satp::Satp::from_bits(self.vmm.gen_satp()));
        debug_assert_ne!(tp.satp.bits(), vma.satp.bits());
        for vpn in vma.iter_vpn() {
            let paddr = vpn
                .translate(vma.satp.ppn(), vma.satp.mode(), &BarePtReader)
                .unwrap();
            let dst_vpn = VirtPageNum::from_vaddr(paddr.0);
            if tp.is_empty() {
                tp = tp.start(dst_vpn);
            }
            tp = tp.size(tp.size + PAGE_SIZE);
            self.vmm.map_frame(
                dst_vpn,
                PhysPageNum::from_paddr(paddr),
                PTEFlags::rx().accessed(),
            );
        }

        tp
    }

    pub fn collect_unused(&mut self) -> (usize, usize) {
        let vma = self.vmm.frame_allocator.vma();
        // link_remain_frame(vma)
        log::debug!("remaining free size: {:#x}", vma.size);

        let mut ll_node = 0;
        for offset in (0..vma.size).step_by(0x1000).rev() {
            let vaddr = VirtAddr(vma.start + offset);
            let paddr = trans_direct(vaddr, vma.satp.ppn(), vma.satp.mode()).unwrap();
            // clean content
            unsafe {
                let slice = core::slice::from_raw_parts_mut(paddr.0 as *mut u8, 0x1000);
                for b in slice {
                    *b = 0;
                }
                *(paddr.0 as *mut usize) = ll_node
            };
            ll_node = paddr.0;
        }
        (ll_node, vma.size)
    }

    pub fn map_vma(&mut self, src: VirtMemArea, dst: VirtMemArea) -> Option<VirtMemArea> {
        let dst = dst.satp(satp::Satp::from_bits(self.vmm.gen_satp()));
        assert_eq!(src.size, dst.size);
        for (v_s, v_d) in src.iter_vpn().zip(dst.iter_vpn()) {
            let ppn = v_s
                .translate(src.satp.ppn(), src.satp.mode(), &BarePtReader)
                .unwrap();
            self.vmm.map_frame(v_d, ppn.into(), dst.flags);
        }
        Some(dst)
    }

    pub fn alloc_vma(&mut self, vma: VirtMemArea) -> Option<VirtMemArea> {
        let vma = vma.satp(satp::Satp::from_bits(self.vmm.gen_satp()));
        self.vmm.alloc_vma(vma)
    }

    pub fn map_frames(&mut self, ppn: PhysPageNum, vma: VirtMemArea) -> VirtMemArea {
        let vma = vma.satp(satp::Satp::from_bits(self.vmm.gen_satp()));
        for (i, vpn) in vma.iter_vpn().enumerate() {
            let ppn = ppn.add(i);
            self.vmm.map_frame(vpn, ppn, vma.flags);
        }

        vma
    }
}

pub mod lse {
    use channel::h2e::LseInfo;
    use riscv::register::satp;
    use vm::prelude::*;

    use super::UserArgs;

    pub fn get_user_args(addr: usize) -> UserArgs {
        let load_info = unsafe {
            let paddr = VirtAddr(addr)
                .translate(satp::read().ppn(), satp::read().mode(), &BarePtReader)
                .unwrap();

            &*(paddr.0 as *const LseInfo)
        };

        UserArgs {
            mem: VirtMemArea::default()
                .start(load_info.mem.start as usize)
                .size(load_info.mem.page_num * PAGE_SIZE),
            rt: VirtMemArea::default()
                .start(load_info.rt.ptr as usize)
                .size(load_info.rt.size),
            ..Default::default()
        }
    }
}

pub mod lue {
    use channel::h2e::LueInfo;
    use console::log;
    use context::SupervisorRegs;
    use device::device::Device;
    use enclave::{Layout, LinuxServiceEnclave, LinuxUserEnclave};
    use riscv::register::satp;
    use sbi::TrapRegs;
    use vm::prelude::*;

    use super::UserArgs;

    pub fn prepare_launch(enc: &mut LinuxUserEnclave, regs: &mut TrapRegs) -> (usize, usize) {
        let sregs = SupervisorRegs::dump();
        enc.nw_ctx.sregs = sregs;
        enc.nw_ctx.tregs = regs.clone();
        // set mepc to the next instruction
        enc.nw_ctx.tregs.mepc += 0x2;

        // prepare satp
        debug_assert_ne!(enc.data.enc_ctx.sregs.satp, 0);
        debug_assert_ne!(enc.data.enc_ctx.sregs.satp, satp::read().bits());
        satp::write(enc.data.enc_ctx.sregs.satp);

        enc.pmp_record.start();

        (enc.data.enc_ctx.tregs.a0, enc.data.enc_ctx.tregs.a1)
    }

    pub fn pause(enc: &mut LinuxUserEnclave, regs: &mut TrapRegs) -> Result<(), enclave::Error> {
        log::debug!("Pausing lue #{}", enc.id().0);
        regs.mepc += 0x4;
        let rc = regs.a0;

        enc.data.enc_ctx.save(&regs);
        log::debug!("saved enclave context:");
        log::debug!("satp: {:#x}", enc.data.enc_ctx.sregs.satp);
        log::debug!("sscratch: {:#x}", enc.data.enc_ctx.sregs.sscratch);
        log::debug!("sstatus: {:#x}", enc.data.enc_ctx.sregs.sstatus);
        log::debug!("stvec: {:#x}", enc.data.enc_ctx.sregs.stvec);
        log::debug!("sepc: {:#x}", enc.data.enc_ctx.sregs.sepc);
        log::debug!("scaues: {:#x}", enc.data.enc_ctx.sregs.scaues);
        log::debug!("stval: {:#x}", enc.data.enc_ctx.sregs.stval);

        enc.data.pmp_cache.dump();
        log::debug!("dumpped pmp entires");

        *regs = unsafe { enc.nw_ctx.restore() };
        regs.a0 = 0;
        regs.a1 = rc;

        enc.data.switch_cycle.end();
        Ok(())
    }

    pub fn create_bootargs(
        bootargs_vma: VirtMemArea,
        mem: VirtMemArea,
        layout: Layout,
        userargs: UserArgs,
        unused_head: usize,
        unused_size: usize,
        device: Device,
    ) {
        use channel::enclave::runtime::*;

        let paddr = bootargs_vma
            .start
            .translate(
                bootargs_vma.satp.ppn(),
                bootargs_vma.satp.mode(),
                &BarePtReader,
            )
            .unwrap();
        let args = unsafe { &mut *(paddr.0 as *mut LueBootArgs) };

        *args = LueBootArgs {
            mem: MemArg {
                total_size: mem.size,
            },
            mods: ModArg {
                start_vaddr: 0,
                num: 0,
            },
            tp: TpArg {
                addr: layout.trampoline.start,
            },
            bin: BinArg {
                start: layout.binary.start,
                size: layout.binary.size,
            },
            shared: SharedArg {
                enc_vaddr: layout.share.start,
                host_vaddr: userargs.share.start as usize,
                size: layout.share.size,
            },
            unmapped: UnmappedArg {
                head: unused_head,
                size: unused_size,
            },
            device,
        };
    }

    pub fn init_layout(args: &UserArgs, lse: &LinuxServiceEnclave) -> enclave::Layout {
        let mut layout = enclave::Layout::default();
        // layout.binary.size = align_up!(args.binary.size, PAGE_SIZE);
        layout.binary.size = args.binary.size;
        // layout.rt.size = align_up!(lse.data.rt.size, PAGE_SIZE);
        layout.rt.size = lse.data.rt.size;
        layout.share.start = align_up!(layout.binary.start + layout.binary.size, PAGE_SIZE);
        // layout.share.size = align_up!(args.share.size, PAGE_SIZE);
        layout.share.size = args.share.size;
        layout.bootargs.size = PAGE_SIZE;

        debug_assert_eq!(
            args.mem.size,
            align_up!(layout.rt.size, PAGE_SIZE)
                + align_up!(layout.binary.size, PAGE_SIZE)
                + align_up!(layout.share.size, PAGE_SIZE)
                + align_up!(args.unused.size, PAGE_SIZE)
                + 0x1000
        );

        layout
    }

    pub fn get_args(addr: usize) -> UserArgs {
        debug_assert_ne!(addr, 0);
        let load_info = unsafe {
            let paddr = VirtAddr(addr)
                .translate(satp::read().ppn(), satp::read().mode(), &BarePtReader)
                .unwrap();

            &*(paddr.0 as *const LueInfo)
        };

        UserArgs {
            mem: VirtMemArea::default()
                .start(load_info.mem.start as usize)
                .size(load_info.mem.page_num * PAGE_SIZE),
            rt: VirtMemArea::default()
                .start(load_info.rt.ptr as usize)
                .size(load_info.rt.size),
            binary: VirtMemArea::default()
                .start(load_info.bin.ptr as usize)
                .size(load_info.bin.size),
            share: VirtMemArea::default()
                .start(load_info.shared.ptr as usize)
                .size(load_info.shared.size),
            unused: VirtMemArea::default()
                .start(load_info.unused.start as usize)
                .size(load_info.unused.size),
        }
    }
}

#[inline]
pub fn measure_data(vma: VirtMemArea) -> md5::Digest {
    let mut ctx = md5::Context::new();

    for vpn in vma.iter_vpn() {
        let paddr = vpn
            .translate(vma.satp.ppn(), vma.satp.mode(), &BarePtReader)
            .unwrap();

        let bytes = unsafe { core::slice::from_raw_parts(paddr.0 as *const u8, 0x1000) };
        ctx.consume(bytes);
    }

    ctx.compute()
}

struct InnerAllocator {
    vma: VirtMemArea,
}

impl InnerAllocator {
    pub fn new(vma: VirtMemArea) -> Self {
        Self { vma }
    }

    pub fn alloc_vpage(&mut self) -> Option<VirtMemArea> {
        if self.vma.size == 0 {
            None
        } else {
            let val = self.vma.start;
            self.vma.start += PAGE_SIZE;
            self.vma.size -= PAGE_SIZE;

            log::trace!("one shot allocator alloc vpage: {:#x}", val);
            Some(
                VirtMemArea::default()
                    .start(val)
                    .size(PAGE_SIZE)
                    .satp(self.vma.satp),
            )
        }
    }

    pub fn alloc_frame(&mut self) -> Option<PhysPageNum> {
        self.alloc_vpage()
            // .map(|vma| VirtAddr::from(vma))
            .and_then(|vma| {
                vma.start
                    .translate(vma.satp.ppn(), vma.satp.mode(), &BarePtReader)
            })
            .map(|paddr| PhysPageNum::from_paddr(paddr))
    }
}

pub struct BuilderAllocator {
    inner: RefCell<InnerAllocator>,
}

impl BuilderAllocator {
    pub fn new(vma: VirtMemArea) -> Self {
        Self {
            inner: RefCell::new(InnerAllocator::new(vma)),
        }
    }

    pub fn vma(&self) -> VirtMemArea {
        self.inner.borrow().vma
    }
}

impl FrameAllocator for BuilderAllocator {
    fn alloc(&self) -> Option<PhysPageNum> {
        let ppn = self.inner.borrow_mut().alloc_frame()?;
        let slice = unsafe { core::slice::from_raw_parts_mut((ppn.0 * 0x1000) as *mut u8, 4096) };
        for b in slice {
            *b = 0;
        }

        Some(ppn)
    }

    fn dealloc(&self, _: PhysPageNum) {
        unimplemented!()
    }
}
