use super::{EnclaveId, HostInfo, LinuxUserEnclave};
use crate::{
    Enclave, EnclaveData, EnclaveInfo,
    lde::{LinuxDriver, LinuxDriverEnclave},
    lse::{LinuxService, LinuxServiceEnclave},
    lue::LinuxUser,
};
use htee_console::log;
use riscv::register::satp;
use vm::{
    Translate,
    allocator::FrameAllocator,
    mm::SV39,
    page_table::{BarePtReader, BarePtWriter, PTEFlags},
    pm::{PhysAddr, PhysPageNum},
    prelude::*,
    trans_direct,
    vm::Sv39VmMgr,
};

pub trait Builder<A: FrameAllocator> {
    fn eid(&mut self, eid: EnclaveId) -> &mut Self;

    fn prepare_vmm(&mut self, allocator: A) -> &mut Self;
    /// Map the first page of runtime to the trampoline page
    fn create_trampoline(&mut self, tramp: PhysAddr) -> &mut Self;

    fn map_frames(
        &mut self,
        ppn: PhysPageNum,
        vpn: VirtPageNum,
        num: usize,
        flags: PTEFlags,
    ) -> &mut Self;

    fn map_host_pages(
        &mut self,
        src: VirtPageNum,
        dst: VirtPageNum,
        num: usize,
        flags: PTEFlags,
    ) -> &mut Self;

    fn map_lse(&mut self, lse: &mut LinuxServiceEnclave, vaddr: VirtAddr) -> &mut Self;
}

// trait Update {
//     fn update_satp(&mut self, satp: satp::Satp);
// }

// impl Update for LinuxUserEnclave {
//     fn update_satp(&mut self, satp: satp::Satp) {
//         self.data.enc_ctx.sregs.satp = satp
//     }
// }

pub struct EncBuilder<'a, D, A>
where
    D: EnclaveData + Setup,
    A: FrameAllocator,
{
    enclave: &'a mut Enclave<D>,
    enc_vmm: Option<Sv39VmMgr<BarePtWriter, A>>,
    stack_ppn: PhysPageNum,
    args_ppn: PhysPageNum,
    tp: usize,
}

impl<'a, D, A> Builder<A> for EncBuilder<'a, D, A>
where
    A: FrameAllocator,
    D: EnclaveData + Setup,
{
    fn eid(&mut self, eid: EnclaveId) -> &mut Self {
        self.enclave.id = eid;
        self
    }

    fn create_trampoline(&mut self, paddr: PhysAddr) -> &mut Self {
        // let rt_tp_ppn = self.translate_host(rt_start).unwrap().floor();
        log::debug!("Trampoline page: {:#x}", paddr.0);
        let vpn = VirtPageNum::from_vaddr(paddr.0 as usize);
        self.enc_vmm.as_mut().unwrap().map_frame(
            vpn,
            PhysPageNum::from_paddr(paddr),
            PTEFlags::rx().accessed(),
        );
        // self.tp = VirtAddr::from_vpn(rt_tp_vpn);
        self.tp = paddr.0;
        self.enclave.tp = self.tp;
        self
    }

    fn prepare_vmm(&mut self, allocator: A) -> &mut Self {
        let pgd = allocator.alloc().unwrap();
        self.stack_ppn = allocator.alloc().unwrap();
        self.args_ppn = allocator.alloc().unwrap();

        let enc_vmm = Sv39VmMgr::new(
            pgd,
            BarePtWriter,
            allocator,
            self.enclave.nw_vma.satp.asid(),
            SV39,
        );

        self.enclave.data.setup_satp(enc_vmm.gen_satp());
        log::debug!("Enclave satp: {:#x}", enc_vmm.gen_satp());
        self.enc_vmm = Some(enc_vmm);
        self
    }

    fn map_frames(
        &mut self,
        ppn: PhysPageNum,
        vpn: VirtPageNum,
        num: usize,
        flags: PTEFlags,
    ) -> &mut Self {
        for i in 0..num {
            let ppn = ppn.add(i);
            let vpn = vpn.add(i);
            log::debug!("mapping frame to enclave: {:#x}->{:#x}", ppn.0, vpn.0);
            self.enc_vmm.as_mut().unwrap().map_frame(vpn, ppn, flags);
        }
        self
    }

    fn map_host_pages(
        &mut self,
        src: VirtPageNum,
        dst: VirtPageNum,
        num: usize,
        flags: PTEFlags,
    ) -> &mut Self {
        log::debug!(
            "mapping host page to enclave: {:#x}->{:#x}, {:#x}",
            src.0,
            dst.0,
            num * 0x1000
        );

        for (src_vpn, dst_vpn) in (src..src.add(num)).zip(dst..dst.add(num)) {
            let ppn = PhysPageNum::from_paddr(self.translate_host(src_vpn).unwrap());
            let vmm = self.enc_vmm.as_mut().unwrap();
            log::trace!("{:#x}: {:#x} -> {:#x}", ppn.0, src_vpn.0, dst_vpn.0,);
            vmm.map_frame(dst_vpn, ppn, flags);
        }

        self
    }

    fn map_lse(&mut self, lse: &mut LinuxServiceEnclave, vaddr: VirtAddr) -> &mut Self {
        log::debug!("map lse to {:#x}", vaddr.0);
        let vmm = self.enc_vmm.as_mut().unwrap();

        let mut lse_addr = lse.data.vma.start;
        let mut target = vaddr.0;
        while lse_addr < lse.data.vma.start + lse.data.vma.size {
            let ppn = PhysPageNum::from_paddr(
                lse_addr
                    .translate(lse.nw_vma.satp.ppn(), lse.nw_vma.satp.mode(), &BarePtReader)
                    .unwrap(),
            );
            let vpn = VirtPageNum::from_vaddr(target);
            vmm.map_frame(vpn, ppn, PTEFlags::rx().accessed());
            lse_addr += 0x1000;
            target += 0x1000;
        }

        self
    }
}

impl<'a, D, A> EncBuilder<'a, D, A>
where
    A: FrameAllocator,
    D: EnclaveData + Setup,
{
    pub fn new(enclave: &'a mut Enclave<D>) -> Self {
        Self {
            enclave,
            enc_vmm: None,
            stack_ppn: PhysPageNum::INVALID,
            args_ppn: PhysPageNum::INVALID,
            tp: 0,
        }
    }

    fn translate_host(&self, vaddr: impl Into<VirtAddr>) -> Option<PhysAddr> {
        use vm::Translate;

        let vaddr: VirtAddr = vaddr.into();
        vaddr.translate(
            self.enclave.nw_vma.satp.ppn(),
            self.enclave.nw_vma.satp.mode(),
            &BarePtReader,
        )
    }

    pub fn finish(&mut self) -> (&'static mut Enclave<D>, A) {
        let enclave = unsafe { &mut *(self.enclave as *mut _) };

        (enclave, self.enc_vmm.take().unwrap().frame_allocator)
    }
}

pub trait Setup {
    fn setup_satp(&mut self, satp: usize);
}

impl Setup for LinuxUser {
    fn setup_satp(&mut self, satp: usize) {
        self.enc_ctx.sregs.satp = satp;
    }
}

impl Setup for LinuxService {
    fn setup_satp(&mut self, _: usize) {
        // self.data.enc_ctx.sregs.satp = satp;
    }
}

impl Setup for LinuxDriver {
    fn setup_satp(&mut self, satp: usize) {
        unimplemented!()
        // self.data.sregs.satp = satp;
    }
}

// impl Setup for LinuxUserEnclave {
//     fn set_host_info(&mut self, host_info: HostInfo) -> &mut Self {
//         self.host_info = host_info;
//         self
//     }

//     fn set_enclave_info(&mut self, enclave_info: EnclaveInfo) -> &mut Self {
//         self.id = enclave_info.eid;
//         self.enclave_ctx.sregs.satp = enclave_info.satp;
//         self.tp = enclave_info.tp;
//         self
//     }
// }

// impl Setup for LinuxDriverEnclave {
//     fn set_host_info(&mut self, host_info: HostInfo) -> &mut Self {
//         self.host_info = host_info;
//         self
//     }

//     fn set_enclave_info(&mut self, enclave_info: EnclaveInfo) -> &mut Self {
//         self.id = enclave_info.eid;
//         self.enclave_ctx.sregs.satp = enclave_info.satp;
//         self.tp = enclave_info.tp;
//         self
//     }
// }

// pub struct EnclaveBuilder<'a, E, A>
// where
//     A: FrameAllocator,
// {
//     pub enclave: &'a mut E,
//     pub enc_vmm: Option<Sv39VmMgr<BarePtWriter, A>>,
//     pub stack_ppn: PhysPageNum,
//     pub args_ppn: PhysPageNum,
//     pub rt_start: usize,
//     pub bin_start: usize,
//     pub bootarg_addr: usize,
//     pub host_info: HostInfo,
//     pub tp: usize,
//     pub eid: EnclaveId,
// }

// impl<'a, E: Setup, A> EnclaveBuilder<'a, E, A>
// where
//     A: FrameAllocator,
// {
//     pub fn new(enclave: &'a mut E) -> Self {
//         use crate::{DEFAULT_BIN_START, DEFAULT_BOOTARG_ADDR, DEFAULT_RT_START};

//         Self {
//             enclave,
//             enc_vmm: None,
//             stack_ppn: PhysPageNum::INVALID,
//             args_ppn: PhysPageNum::INVALID,
//             rt_start: DEFAULT_RT_START,
//             bin_start: DEFAULT_BIN_START,
//             bootarg_addr: DEFAULT_BOOTARG_ADDR,
//             host_info: HostInfo::default(),
//             tp: 0,
//             eid: EnclaveId::new(),
//         }
//     }

//     pub fn eid(mut self, eid: EnclaveId) -> Self {
//         self.eid = eid;
//         self
//     }

//     fn translate_host(&self, vaddr: impl Into<VirtAddr>) -> Option<PhysAddr> {
//         use vm::Translate;

//         let vaddr: VirtAddr = vaddr.into();
//         vaddr.translate(
//             self.host_info.pt_root,
//             self.host_info.pt_mode,
//             &BarePtReader,
//         )
//     }

//     pub fn host_info(self, host_info: HostInfo) -> Self {
//         Self { host_info, ..self }
//     }

//     pub fn prepare_vmm(mut self, allocator: A) -> Self {
//         let pgd = allocator.alloc().unwrap();
//         self.stack_ppn = allocator.alloc().unwrap();
//         self.args_ppn = allocator.alloc().unwrap();

//         let enc_vmm = Sv39VmMgr::new(pgd, BarePtWriter, allocator, self.host_info.asid, SV39);
//         log::debug!("Enclave satp: {:#x}", enc_vmm.gen_satp());
//         self.enc_vmm = Some(enc_vmm);
//         self
//     }

//     /// Map the first page of runtime to the trampoline page
//     pub fn create_trampoline(mut self, rt_start: VirtAddr) -> Self {
//         let rt_tp_ppn = self.translate_host(rt_start).unwrap().floor();
//         log::debug!("Trampoline page: {:#x}", rt_tp_ppn.0);
//         let rt_tp_vpn = VirtPageNum(rt_tp_ppn.0);
//         self.enc_vmm
//             .as_mut()
//             .unwrap()
//             .map_frame(rt_tp_vpn, rt_tp_ppn, PTEFlags::rwx());
//         // self.tp = VirtAddr::from_vpn(rt_tp_vpn);
//         self.tp = rt_tp_vpn.0 * 0x1000;
//         self
//     }

//     pub fn map_mmio(mut self, mmio: &Range<usize>) -> Self {
//         if mmio.len() == 0 {
//             return self;
//         }
//         let vmm = self.enc_vmm.as_mut().unwrap();
//         log::debug!("map mmio area: {:#x}-{:#x}", mmio.start, mmio.end);
//         let start_vpn = VirtPageNum::from_vaddr(mmio.start);
//         let start_ppn = PhysPageNum::from_paddr(mmio.start);
//         let num = (mmio.end - mmio.start + 0xfff) / 0x1000;

//         vmm.map_frames(start_vpn, start_ppn, num, PTEFlags::rwx());
//         self
//     }

//     pub fn map_mods(mut self, mods_start: VirtAddr, mods_len: usize, dst: VirtAddr) -> Self {
//         let paddr = self.translate_host(mods_start).unwrap();
//         let mods = unsafe { slice::from_raw_parts(paddr.0 as *const ModInfo, mods_len) };
//         let mut dst_vpn = VirtPageNum::from_vaddr(dst);
//         for m in mods {
//             let page_num = m.size.div_ceil(0x1000);
//             self = self.map_host_pages(
//                 VirtPageNum::from_vaddr(m.ptr as usize),
//                 dst_vpn,
//                 page_num,
//                 PTEFlags::rwx(),
//             );
//             dst_vpn = dst_vpn.add(page_num);
//         }
//         self
//     }

//     pub fn map_host_pages(
//         mut self,
//         src: VirtPageNum,
//         dst: VirtPageNum,
//         num: usize,
//         flags: PTEFlags,
//     ) -> Self {
//         log::debug!(
//             "mapping host page to enclave: {:#x}->{:#x}, {:#x}",
//             src.0,
//             dst.0,
//             num * 0x1000
//         );

//         for (src_vpn, dst_vpn) in (src..src.add(num)).zip(dst..dst.add(num)) {
//             let ppn = PhysPageNum::from_paddr(self.translate_host(src_vpn).unwrap());
//             let vmm = self.enc_vmm.as_mut().unwrap();
//             // log::trace!("{:#x}: {:#x} -> {:#x}", ppn.0, src_vpn.0, dst_vpn.0,);
//             vmm.map_frame(dst_vpn, ppn, flags);
//         }

//         self
//     }

//     pub fn map_frame(mut self, ppn: PhysPageNum, vpn: VirtPageNum, flags: PTEFlags) -> Self {
//         log::debug!("mapping frame to enclave: {:#x}->{:#x}", ppn.0, vpn.0);
//         let vmm = self.enc_vmm.as_mut().unwrap();
//         vmm.map_frame(vpn, ppn, flags);
//         self
//     }

//     pub fn finish(mut self) -> (&'a mut E, VirtMemMgr<BarePtWriter, A, SV39>) {
//         let vmm = core::mem::take(&mut self.enc_vmm).unwrap();
//         let host_satp = vmm.update_satp();
//         let enclave = self.enclave;
//         enclave
//             .set_enclave_info(EnclaveInfo {
//                 eid: self.eid,
//                 satp: satp::read(),
//                 tp: self.tp,
//             })
//             .set_host_info(self.host_info);
//         satp::write(host_satp.bits());
//         (enclave, vmm)
//     }
// }

pub fn link_remain_frame(start: usize, size: usize, host_satp: satp::Satp) -> (usize, usize) {
    log::debug!("remaining free size: {size:#x}");

    let mut ll_node = 0;
    for offset in (0..size).step_by(0x1000).rev() {
        let vaddr = VirtAddr(start + offset);
        let paddr = trans_direct(vaddr, host_satp.ppn(), host_satp.mode()).unwrap();
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
    (ll_node, size)
}
