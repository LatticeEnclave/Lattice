use core::{fmt::{Debug, Formatter}, marker::PhantomData,};

use alloc::vec::Vec;
use vm::{page_table::{PTEFlags}, pm::PhysPageNum, vm::{VirtAddr, VirtPageNum}, PageTableReader};

use crate::{consts::{PAGE_SIZE, USER_BUFFER}, kernel::{self, LinuxUserKernel}};

/// Raw pointer from user space.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct UsrPtr<T, P: Policy>(*mut T, PhantomData<P>);

/// marks the raw pointer's behavior.
pub trait Policy {}

/// marks a pointer used to read.
pub trait Read: Policy {}

/// marks a pointer used to write.
pub trait Write: Policy {}

/// type for user pointer used to read.
pub struct In;

/// type for user pointer used to write.
pub struct Out;

/// type for user pointer used to write and read.
pub struct InOut;

impl Policy for In {}
impl Policy for Out {}
impl Policy for InOut {}
impl Read for In {}
impl Write for Out {}
impl Read for InOut {}
impl Write for InOut {}

pub type UsrInPtr<T> = UsrPtr<T, In>;
pub type UsrOutPtr<T> = UsrPtr<T, Out>;
pub type UsrInOutPtr<T> = UsrPtr<T, InOut>;

/// The error type which is returned from user pointer operation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidUtf8,
    InvalidPointer,
    BufferTooSmall,
    InvalidLength,
    InvalidVectorAddress,
}

type Result<T> = core::result::Result<T, Error>;

// impl<T, P: Policy> Debug for UsrPtr<T, P> {
//     fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
//         // print the user raw pointer
//         write!(f, "{:?}", self.0)
//     }
// }


impl<T, P: Policy> From<usize> for UsrPtr<T, P> {
    fn from(ptr: usize) -> Self {
        UsrPtr(ptr as _, PhantomData)
    }
}

impl<T, P: Policy> UsrPtr<T, P> {
    ///get the pointer to the buf in runtime space by user pointer
    pub fn from_usr_buf(usr_buf: &UsrBuf) -> Self{
        let offset = usr_buf.addr % PAGE_SIZE;
        let rt_buf_addr = usr_buf.shared_addr + offset;
        Self::from(rt_buf_addr)
    }

    /// checks if 'size' fits a 'T'.
    /// construct a user pointer from 'addr'.
    pub fn from_addr_size(addr: usize, size: usize) -> Result<Self> {
        if size >= core::mem::size_of::<T>() {
            Ok(Self::from(addr))
        } else {
            Err(Error::BufferTooSmall)
        }
    }

    /// Returns 'true' if pointer is null.
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    /// add the offset of the user pointer
    pub fn add(&self, count: usize) -> Self {
        Self(unsafe { self.0.add(count) }, PhantomData)
    }

    /// checks legality of the user pointer.
    /// Returns ['Ok(())'] if it is neither null nor unaligned.
    pub fn check(&self) -> Result<()> {
        if !self.0.is_null() && (self.0 as usize) % core::mem::align_of::<T>() == 0 {
            Ok(())
        } else {
            Err(Error::InvalidPointer)
        }
    }
}

impl<T, P: Read> UsrPtr<T, P> {
    /// Reads the value from 'self'
    pub fn read(&self) -> Result<T> {
        self.check()?;
        Ok(unsafe {self.0.read()})
    }

    /// Forms a pointer from a user pointer and a 'len'
    pub fn as_slice(&self, len: usize) -> Result<&'static [T]> {
        if len == 0 {
            Ok(&[])
        } else {
            self.check()?;
            Ok(unsafe {core::slice::from_raw_parts(self.0, len)})
        }
    }
}

impl<T, P: Write> UsrPtr<T, P> {
    pub fn write(&mut self, value: T) -> Result<()> {
        self.check()?;
        unsafe {self.0.write(value);}
        Ok(())
    }

    pub fn write_array(&mut self, values: &[T]) -> Result<()> {
        if !values.is_empty() {
            self.check()?;
            unsafe {
                self.0
                    .copy_from_nonoverlapping(values.as_ptr(), values.len());
            };
        }
        Ok(())
    }
}

impl<P: Read> UsrPtr<u8, P> {
    /// Forms a utf-8 string slice from a user pointer and a 'len'.
    pub fn as_str(&self, len: usize) -> Result<&'static str> {
        core::str::from_utf8(self.as_slice(len)?).map_err(|_| Error::InvalidUtf8)
    }

    /// Forms a zero-terminated string slice from a user pointer to a c style string.
    pub fn as_c_str(&self) -> Result<&'static str> {
        self.as_str(unsafe {(0usize..).find(|&i| *self.0.add(i) == 0).unwrap()})
    }
}

impl<P: Write> UsrPtr<u8, P> {
    /// copies 's' to pointer and write a '\0' as a C style string.
    pub fn write_cstring(&mut self, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        self.write_array(bytes)?;
        unsafe {self.0.add(bytes.len()).write(0);};
        Ok(())
    }
}

pub enum Buf_Policy {
    Write,
    Read,
    ReadWrite,
}

/// map the user space to the kernel space to read and write by pointer
/// len: bytes nums
pub struct UsrBuf {
    addr: usize,
    len: usize,
    shared_addr: usize,
    policy: Buf_Policy,
}

impl UsrBuf {
    pub fn new(addr: usize, len: usize, policy: Buf_Policy, shared_op_addr: Option<usize>) -> Self {
        let shared_addr = shared_op_addr.unwrap_or(USER_BUFFER);
        Self{
            addr,
            len,
            shared_addr,
            policy,
        }
    }

    fn get_flag(&self) -> PTEFlags {
        match self.policy {
            Buf_Policy::Write => PTEFlags::W,
            Buf_Policy::Read => PTEFlags::R,
            Buf_Policy::ReadWrite => PTEFlags::W | PTEFlags::R,
        }
    }

    pub fn build_buf(&self) {
        let p_num: usize = ((self.addr + self.len + PAGE_SIZE) / PAGE_SIZE) - (self.addr / PAGE_SIZE);
        let kernel = unsafe { LinuxUserKernel::from_sscratch() };
        for i in 0..p_num {
            let user_vpn = VirtPageNum::from_vaddr(self.addr + i * PAGE_SIZE);
            let rt_vpn = VirtPageNum::from_vaddr(self.shared_addr + i * PAGE_SIZE);
            let ppn = kernel
                            .task
                            .vmm
                            .lock()
                            .get_pte(user_vpn)
                            .unwrap()
                            .get_ppn();

            
            let _ = kernel
                    .vmm
                    .lock()
                    .map_frame(rt_vpn, ppn, PTEFlags::rwx().accessed().dirty());
        }
        unsafe {
            sbi::tlb_flush!();
        }  
    }

    pub fn retract_buf(&self) {
        let p_num: usize = ((self.addr + self.len + PAGE_SIZE) / PAGE_SIZE) - (self.addr / PAGE_SIZE);
        let kernel = unsafe { LinuxUserKernel::from_sscratch() };         
        let _ = kernel
                .vmm
                .lock()
                .unmap_vma(self.shared_addr, PAGE_SIZE * p_num);
        unsafe {
            sbi::tlb_flush!();
        }            
    }
}

pub unsafe fn copy_from_user(usr_buf: UsrBuf, shared_ptr: usize) {
    usr_buf.build_buf();

    // copy data to the user
    let p: UsrInPtr<u8> = UsrPtr::from_usr_buf(&usr_buf);
    let shared_ptr = shared_ptr as *mut u8;
    for i in 0..usr_buf.len {
        let ch= p.add(i).read().unwrap();
        let _ = shared_ptr.add(i).write(ch);
    }

    usr_buf.retract_buf();
}

pub unsafe fn copy_cstring_from_user(usr_buf: UsrBuf, shared_ptr: usize) -> usize {
    // get the c_string
    usr_buf.build_buf();
    let p: UsrInPtr<u8> = UsrInPtr::from_usr_buf(&usr_buf);
    let c_str = p.as_c_str().unwrap();

    let len = c_str.len();
    (shared_ptr as *mut u8).copy_from_nonoverlapping(c_str.as_ptr(), len);

    usr_buf.retract_buf();
    len
}

pub unsafe fn copy_to_user(usr_buf: UsrBuf, shared_ptr: usize) {
    usr_buf.build_buf();

    // copy data to the user
    let mut p: UsrOutPtr<u8> = UsrPtr::from_usr_buf(&usr_buf);
    let shared_ptr = shared_ptr as *mut u8;
    let array = core::slice::from_raw_parts(shared_ptr, usr_buf.len);
    let _ = p.write_array(array);
    
    usr_buf.retract_buf();    
}