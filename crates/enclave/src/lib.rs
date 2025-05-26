#![no_std]
#![feature(never_type)]

// use builder::{EncBuilder, Setup};
use context::HartContext;
use console::log;
use lue::LinuxUser;
use perf::PmpFaultRecord;
use pma::Owner;
use riscv::register::satp;
use spin::Mutex;
use vm::{PAGE_SIZE, VirtMemArea, align_down, pm::PhysPageNum, vm::VirtAddr};

use core::{fmt::Display, ptr::NonNull, sync::atomic::AtomicUsize};

mod layout;
mod lse;
mod lue;
mod node;

pub mod prelude {
    // pub use crate::builder
}

// pub use lde::LinuxDriverEnclave;
// pub use builder::{Builder, link_remain_frame};
pub use layout::Layout;
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

pub fn create_lue_at(addr: usize, eid: EnclaveId) -> &'static mut LinuxUserEnclave {
    let enc = Enclave::create_at(addr);
    enc.list.lock().value = EnclaveType::User;
    enc.nw_vma = enc.nw_vma.satp(satp::read());
    enc.id = eid;
    enc
}

pub fn create_lse_at(addr: usize, eid: EnclaveId) -> &'static mut LinuxServiceEnclave {
    let enc = Enclave::create_at(addr);
    enc.list.lock().value = EnclaveType::Service;
    enc.nw_vma = enc.nw_vma.satp(satp::read());
    enc.id = eid;
    enc
}

/// Self reference
#[repr(C)]
pub struct Enclave<D: EnclaveData> {
    list: Mutex<EncListNode>,

    id: EnclaveId,

    pub nw_vma: VirtMemArea,
    pub nw_ctx: HartContext,

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
    #[inline]
    pub fn as_enc<D: EnclaveData>(&self) -> Option<&'static mut Enclave<D>> {
        if self.get_type() == D::TYPE {
            let idx = self.idx();
            // SAFETY: We have checked the type of the enclave.
            Some(unsafe { Enclave::from_ptr(idx.0) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_lue(&self) -> Option<&'static mut Enclave<LinuxUser>> {
        self.as_enc::<LinuxUser>()
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
