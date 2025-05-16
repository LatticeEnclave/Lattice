#![no_std]
#![feature(never_type)]

use builder::{EncBuilder, Setup};
use context::HartContext;
use htee_console::log;
use lue::LinuxUser;
use perf::{PmpFaultRecord, TimeRecord};
use pma::Owner;
use riscv::{
    asm::sfence_vma_all,
    register::{mepc, mstatus, satp, sstatus},
};
use spin::Mutex;
use vm::{
    PAGE_SIZE, align_down, aligned,
    allocator::FrameAllocator,
    pm::{PhysAddr, PhysPageNum},
    vm::VirtAddr,
};

use core::{arch::asm, fmt::Display, ops::Range, ptr::NonNull, sync::atomic::AtomicUsize};

mod builder;
mod ctl;
pub mod ecall;
mod lde;
mod lse;
mod lue;
mod node;

// pub use lde::LinuxDriverEnclave;
pub use builder::{Builder, link_remain_frame};
pub use lse::{LinuxServiceEnclave, LinuxServiceEnclaveList};
pub use lue::{LinuxUserEnclave, LinuxUserEnclaveList};
pub use node::EncListNode;

pub const DEFAULT_RT_START: usize = 0xFFFF_FFFF_8000_0000;
pub const DEFAULT_BIN_START: usize = 0x20_0000_0000;
pub const DEFAULT_BOOTARG_ADDR: usize = 0xFFFF_FFFF_7FF0_0000;
pub trait EnclaveData {
    const TYPE: EnclaveType;
}

impl EnclaveData for () {
    const TYPE: EnclaveType = EnclaveType::None;
}

/// Self reference
#[repr(C)]
pub struct Enclave<D: EnclaveData> {
    list: Mutex<EncListNode>,

    id: EnclaveId,

    pub normal_region: Range<usize>,
    pub normal_satp: satp::Satp,
    pub normal_ctx: HartContext,

    pub tp: usize,

    // Records
    // pub time_record: TimeRecord,
    pub pmp_record: PmpFaultRecord,

    // private data
    data_ptr: *mut D,
    pub data: D,
}

impl<D: EnclaveData> Enclave<D> {
    /// should only be used to create Enclave
    pub fn create_at(addr: usize) -> &'static mut Self {
        debug_assert_eq!(addr & (PAGE_SIZE - 1), 0);
        let enclave: &'static mut Self = unsafe { &mut *(addr as *mut Self) };
        enclave.list = Mutex::new(EncListNode::new(D::TYPE));
        enclave.data_ptr = &mut enclave.data as *mut _;

        enclave.pmp_record = PmpFaultRecord::empty();

        enclave
    }

    #[inline]
    pub fn get_type(&self) -> EnclaveType {
        self.list.lock().value
    }

    /// User must ensure the type of the enclave is correct.
    #[inline]
    pub unsafe fn from_ptr(ptr: impl Into<NonNull<EncListNode>>) -> &'static mut Enclave<D> {
        let ptr = align_down!(ptr.into().as_ptr() as usize, 0x1000);
        let ptr = ptr as *mut Enclave<D>;
        let enc = unsafe { &mut *ptr };
        debug_assert_ne!(enc.id, EnclaveId::HOST);
        enc
    }

    #[inline]
    pub fn idx(&self) -> EnclaveIdx {
        EnclaveIdx(NonNull::new(&self.list as *const _ as *mut _).unwrap())
    }

    #[inline]
    pub fn id(&self) -> EnclaveId {
        self.id
    }

    #[inline]
    pub fn print_records(&mut self) {
        // log::info!("time record: {:#x}", self.time_record.end().get_data());
        log::info!("pmp fault num: {}", self.pmp_record.num);
    }
}

impl Enclave<()> {
    pub fn as_enc<D: EnclaveData>(&self) -> Option<&'static mut Enclave<D>> {
        if self.get_type() == D::TYPE {
            let idx = self.idx();
            // SAFETY: We have checked the type of the enclave.
            Some(unsafe { Enclave::from_ptr(idx.0) })
        } else {
            None
        }
    }
}

pub enum Error {
    InvalidEnclaveType = 1,
    InvalidEnclaveId = 2,
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidEnclaveType => write!(f, "Invalid enclave type"),
            Self::InvalidEnclaveId => write!(f, "Invalid enclave id"),
        }
    }
}

pub fn builder<A: FrameAllocator, D: EnclaveData + Setup>(
    meta_page: PhysAddr,
) -> EncBuilder<'static, D, A> {
    let addr = meta_page.0;
    let enclave = Enclave::create_at(addr);
    enclave.list.lock().value = D::TYPE;
    enclave.normal_satp = satp::read();
    EncBuilder::new(enclave)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnclaveType {
    None = 0,
    User = 1,
    Driver = 2,
    Service = 3,
}

impl EnclaveType {
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User)
    }

    pub fn is_driver(&self) -> bool {
        matches!(self, Self::Driver)
    }

    pub fn to_user(&mut self) {
        *self = Self::User;
    }

    pub fn to_driver(&mut self) {
        *self = Self::Driver;
    }
}

pub type EnclaveId = Owner;

// /// And we use 0 to identify normal world.
// #[derive(Clone, Copy, PartialEq, Eq)]
// pub struct EnclaveIdx(pub usize);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EnclaveIdx(NonNull<EncListNode>);

impl EnclaveIdx {
    /// User must ensure the type of the enclave is correct.
    pub fn as_enc(&self) -> &'static mut Enclave<()> {
        // SAFETY: Enclave<()> is safe to be converted by EnclaveIdx
        unsafe { Enclave::from_ptr(self.0) }
    }
}

impl core::fmt::Display for EnclaveIdx {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0.as_ptr() as usize))
    }
}

impl Into<NonNull<EncListNode>> for EnclaveIdx {
    fn into(self) -> NonNull<EncListNode> {
        self.0
    }
}

impl Into<NonNull<u8>> for EnclaveIdx {
    fn into(self) -> NonNull<u8> {
        NonNull::new(self.0.as_ptr() as *mut _).unwrap()
    }
}

// impl<D: EnclaveData> Into<&'static mut Enclave<D>> for EnclaveIdx {
//     fn into(self) -> &'static mut Enclave<D> {
//         unsafe { Enclave::from_ptr(self) }
//     }
// }

impl From<NonNull<u8>> for EnclaveIdx {
    fn from(value: NonNull<u8>) -> Self {
        Self(NonNull::new(value.as_ptr() as *mut _).unwrap())
    }
}

pub struct EnclaveIdGenerator {
    counter: AtomicUsize,
}

impl EnclaveIdGenerator {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(EnclaveId::START.0),
        }
    }

    pub fn fetch(&self) -> EnclaveId {
        let id = self
            .counter
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        Owner(id)
    }
}

pub struct EnclaveInfo {
    pub eid: EnclaveId,
    pub satp: satp::Satp,
    pub tp: usize,
}

impl Default for EnclaveInfo {
    fn default() -> Self {
        Self {
            eid: EnclaveId::HOST,
            satp: satp::read(),
            tp: 0,
        }
    }
}

pub struct HostInfo {
    pub vaddr: VirtAddr,
    pub size: usize,
    pub asid: usize,
    pub shared_start: VirtAddr,
    pub shared_size: usize,
    pub pt_root: PhysPageNum,
    pub pt_mode: satp::Mode,
}

impl Default for HostInfo {
    fn default() -> Self {
        Self {
            vaddr: VirtAddr::INVALID,
            size: 0,
            asid: 0,
            shared_start: VirtAddr::INVALID,
            shared_size: 0,
            pt_root: PhysPageNum::INVALID,
            pt_mode: satp::Mode::Sv39,
        }
    }
}

mod allocator {
    use core::cell::RefCell;

    use htee_console::log;
    use riscv::register::satp;
    use vm::{allocator::FrameAllocator, prelude::*};

    struct InnerAllocator {
        pub root_ppn: usize,
        pub mode: satp::Mode,
        pub start: usize,
        // pub size: usize,
        pub end: usize,
    }

    impl InnerAllocator {
        pub fn new(root_ppn: usize, mode: satp::Mode, start: usize, size: usize) -> Self {
            Self {
                root_ppn,
                mode,
                start,
                // size
                end: start + size,
            }
        }

        pub fn alloc_vpage(&mut self) -> Option<usize> {
            if self.start == self.end {
                None
            } else {
                let val = self.start;
                self.start += 0x1000;

                log::trace!("one shot allocator alloc vpage: {:#x}", val);
                Some(val)
            }
        }

        pub fn alloc_frame(&mut self) -> Option<PhysPageNum> {
            self.alloc_vpage()
                .map(|addr| VirtAddr::from(addr))
                .and_then(|vaddr| vaddr.translate(self.root_ppn, self.mode, &BarePtReader))
                .map(|paddr| PhysPageNum::from_paddr(paddr))
        }
    }

    pub struct BuilderAllocator {
        inner: RefCell<InnerAllocator>,
    }

    impl BuilderAllocator {
        pub fn new(root_ppn: usize, mode: satp::Mode, start: usize, size: usize) -> Self {
            Self {
                inner: RefCell::new(InnerAllocator::new(root_ppn, mode, start, size)),
            }
        }

        pub fn start(&self) -> usize {
            self.inner.borrow().start
        }

        pub fn size(&self) -> usize {
            self.inner.borrow().end - self.inner.borrow().start
        }
    }

    impl FrameAllocator for BuilderAllocator {
        fn alloc(&self) -> Option<PhysPageNum> {
            let ppn = self.inner.borrow_mut().alloc_frame()?;
            let slice =
                unsafe { core::slice::from_raw_parts_mut((ppn.0 * 0x1000) as *mut u8, 4096) };
            for b in slice {
                *b = 0;
            }

            Some(ppn)
        }

        fn dealloc(&self, _: PhysPageNum) {
            unimplemented!()
        }
    }
}

// fn launch_runtime(addr: usize, sp: usize, arg0: usize, arg1: usize) -> ! {
//     unsafe {
//         mstatus::set_mpp(mstatus::MPP::Supervisor);
//         sstatus::clear_sie(); // disable SIE
//         mepc::write(addr);
//         sfence_vma_all();

//         asm!(
//             "mv     sp, {}",
//             "mret",
//             in(reg) sp,
//             in("a0") arg0,
//             in("a1") arg1,
//             options(noreturn),
//         );
//     }
// }
