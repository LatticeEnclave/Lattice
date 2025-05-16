use core::mem;

use crate::{log, println};
use alloc::collections::btree_map::Values;
use htee_vstack::{ArgRegs, Vstack};

use crate::{
    consts::{SHARED_MEMORY_REGION_SIZE, USER_BUFFER},
    error,
    kernel::LinuxUserKernel,
    syscall::linux_wrap::{SYS_FCNTL, SYS_OPENAT, SYS_UNLINKAT},
    usr::{
        copy_cstring_from_user, copy_from_user, copy_to_user, Buf_Policy, UsrBuf, UsrInPtr,
        UsrOutPtr, UsrPtr,
    },
};

use super::{
    linux_wrap::{
        SYS_CLOSE, SYS_EPOLL_CREATE1, SYS_EPOLL_CTL, SYS_EPOLL_PWAIT, SYS_FSTAT, SYS_FSYNC,
        SYS_FTRUNCATE, SYS_GETCWD, SYS_IOCTL, SYS_LSEEK, SYS_PIPE2, SYS_READ, SYS_SYNC, SYS_WRITE,
    },
    sbi_close_channel, sbi_open_channel, sbi_recv_channel, sbi_stop_enclave,
    time::TimeSpec,
};

#[cfg(target_arch = "riscv64")]
#[derive(Default)]
#[repr(C)]
pub struct IOvec {
    pub iov_base: usize,
    pub iov_len: usize,
}

#[cfg(target_arch = "riscv64")]
#[derive(Default)]
#[repr(C)]
pub struct EpollEvent {
    events: u32,
    data: usize,
}

#[cfg(target_arch = "riscv64")]
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    dev: u64,
    /// inode number
    ino: u64,
    /// number of hard links
    nlink: u64,

    /// file type and mode
    mode: u32,
    /// user ID of owner
    uid: u32,
    /// group ID of owner
    gid: u32,
    /// padding
    _pad0: u32,
    /// device ID (if special file)
    rdev: u64,
    /// total size, in bytes
    size: u64,
    /// blocksize for filesystem I/O
    blksize: u64,
    /// number of 512B blocks allocated
    blocks: u64,

    /// last access time
    atime: TimeSpec,
    /// last modification time
    mtime: TimeSpec,
    /// last status change time
    ctime: TimeSpec,
}

pub fn sys_read(fd: usize, buf: usize, len: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - len;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: fd,
        a1: fixup_ptr,
        a2: len,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_READ,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    if ret != 0 {
        return -1;
    }

    // build buffer
    let rt_buf = UsrBuf::new(buf, len, Buf_Policy::Write, None);
    unsafe { copy_to_user(rt_buf, enc_ptr) };

    ret
}

#[inline(never)]
pub fn sys_write(fd: usize, buf: usize, len: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - (((len + 1) / 2) * 2);
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: fd,
        a1: fixup_ptr,
        a2: len,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_WRITE,
    };
    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    // build buffer
    let rt_buf = UsrBuf::new(buf, len, Buf_Policy::Read, None);
    unsafe {
        copy_from_user(rt_buf, enc_ptr);
    }

    // let ret = len as isize;
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    // log::info!("")
    // println!("len: {}", len);
    // println!("ret: {}", ret);
    // let ret = 0;

    ret
}

pub fn sys_readv(fd: usize, iov: usize, iovcnt: usize) -> isize {
    let mut ret = 0;
    // build buffer
    let rt_buf = UsrBuf::new(
        iov,
        core::mem::size_of::<IOvec>() * iovcnt,
        Buf_Policy::Read,
        Some(USER_BUFFER + SHARED_MEMORY_REGION_SIZE),
    );

    rt_buf.build_buf();
    let iov_p: UsrInPtr<IOvec> = UsrInPtr::from_usr_buf(&rt_buf);
    for i in 0..iovcnt {
        let iovec = iov_p.add(i).read().unwrap();
        ret += sys_read(fd, iovec.iov_base, iovec.iov_len);
    }

    rt_buf.retract_buf();

    ret
}

pub fn sys_writev(fd: usize, iov: usize, iovcnt: usize) -> isize {
    let mut ret = 0;
    // build buffer
    let rt_buf = UsrBuf::new(
        iov,
        core::mem::size_of::<IOvec>() * iovcnt,
        Buf_Policy::Read,
        Some(USER_BUFFER + SHARED_MEMORY_REGION_SIZE),
    );

    rt_buf.build_buf();
    let iov_p: UsrInPtr<IOvec> = UsrInPtr::from_usr_buf(&rt_buf);
    for i in 0..iovcnt {
        let iovec = iov_p.add(i).read().unwrap();
        ret += sys_write(fd, iovec.iov_base, iovec.iov_len);
    }

    rt_buf.retract_buf();

    ret
}

pub fn sys_openat(dirfd: usize, path: usize, flags: isize, mode: usize) -> isize {
    log::debug!(
        "[openat]: openat file fd: {:#x}, open flag: {:#x}, open mode: {:#x}",
        dirfd,
        flags,
        mode
    );

    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();

    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    const MAX_FILE_NAME: usize = 255;
    // build buffer
    let rt_buf = UsrBuf::new(path, MAX_FILE_NAME, Buf_Policy::Read, None);

    let len = unsafe { copy_cstring_from_user(rt_buf, enc_ptr) };

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: dirfd as usize,
        a1: fixup_ptr,
        a2: flags as usize,
        a3: mode,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_OPENAT,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);

    log::debug!("[openat]: return: {:#x}", ret);

    ret
}

pub fn sys_unlinkat(dirfd: usize, path: usize, flags: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    const MAX_FILE_NAME: usize = 255;
    // build buffer
    let rt_buf = UsrBuf::new(path, MAX_FILE_NAME, Buf_Policy::Read, None);

    let len = unsafe { copy_cstring_from_user(rt_buf, enc_ptr) };

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: dirfd as usize,
        a1: fixup_ptr,
        a2: flags as usize,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_UNLINKAT,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);

    ret
}

pub fn sys_newfstatat(dirfd: usize, pathname: usize, statbuf: usize, flags: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();

    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    const MAX_FILE_NAME: usize = 255;
    // build buffer
    let rt_buf = UsrBuf::new(pathname, MAX_FILE_NAME, Buf_Policy::Read, None);

    let len = unsafe { copy_cstring_from_user(rt_buf, enc_ptr) };

    // set stat aligned ptr
    let stat_ptr =
        (enc_ptr + len + core::mem::size_of::<Stat>() - 1) & !(core::mem::size_of::<Stat>() - 1);
    let stat_fixed_ptr = stat_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: dirfd as usize,
        a1: fixup_ptr,
        a2: stat_fixed_ptr,
        a3: flags,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_OPENAT,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);

    if ret == 0 {
        // build buffer
        let rt_buf = UsrBuf::new(
            statbuf,
            core::mem::size_of::<Stat>(),
            Buf_Policy::Write,
            None,
        );
        unsafe { copy_to_user(rt_buf, stat_fixed_ptr) };
    }

    ret
}

pub fn sys_fstat(fd: usize, buf: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);
    let len = core::mem::size_of::<Stat>();

    let enc_ptr = pre_vstack.sp() - len;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: fd,
        a1: fixup_ptr,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_FSTAT,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);

    if ret == 0 {
        // build buffer
        let rt_buf = UsrBuf::new(buf, len, Buf_Policy::Write, None);
        if fd == 1 {
            let fstat = unsafe { &mut *(enc_ptr as *mut Stat) };
            fstat.mode |= 0o620;
        }
        // log::debug!(
        //     "[sys_fstat]: the stat dev is {:#x}, ino is {:#x}",
        //     fstat.dev,
        //     fstat.ino
        // );
        unsafe { copy_to_user(rt_buf, enc_ptr) };
    }

    ret
}

pub fn sys_pipe2(fds: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);
    let len = core::mem::size_of::<i32>() * 2;

    let enc_ptr = pre_vstack.sp() - len;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: fixup_ptr,
        a1: 0,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_PIPE2,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    if ret == 0 {
        // build buffer
        let rt_buf = UsrBuf::new(fds, len, Buf_Policy::Write, None);
        unsafe { copy_to_user(rt_buf, enc_ptr) };
    }

    ret
}

pub fn sys_lseek(fd: usize, offset: usize, whence: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: fd,
        a1: offset,
        a2: whence,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_LSEEK,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_ftruncate(fd: usize, offset: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: fd,
        a1: offset,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_FTRUNCATE,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_sync() -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: 0,
        a1: 0,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_SYNC,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_fsync(fd: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: fd,
        a1: 0,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_FSYNC,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_close(fd: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: fd,
        a1: 0,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_CLOSE,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_epoll_create1(flags: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    let args = ArgRegs {
        a0: flags,
        a1: 0,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_EPOLL_CREATE1,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_epoll_ctl(epfd: usize, op: usize, fd: usize, event: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);
    let len = core::mem::size_of::<EpollEvent>();

    let enc_ptr = pre_vstack.sp() - len;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: epfd,
        a1: op,
        a2: fd,
        a3: fixup_ptr,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_EPOLL_CTL,
    };
    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    // build buffer
    let rt_buf = UsrBuf::new(event, len, Buf_Policy::Read, None);
    unsafe {
        copy_from_user(rt_buf, enc_ptr);
    }

    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_epoll_pwait(epfd: usize, events: usize, maxevents: usize, timeout: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);
    let len = core::mem::size_of::<EpollEvent>();

    let enc_ptr = pre_vstack.sp() - len;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(len, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: epfd,
        a1: fixup_ptr,
        a2: maxevents,
        a3: timeout,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_EPOLL_PWAIT,
    };
    unsafe { set_proxy_syscall_args(pre_vstack, args) };

    // build buffer
    let rt_buf = UsrBuf::new(events, len, Buf_Policy::Read, None);
    unsafe {
        copy_from_user(rt_buf, enc_ptr);
    }

    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    // build buffer
    let rt_buf = UsrBuf::new(events, len, Buf_Policy::Write, None);
    unsafe {
        copy_to_user(rt_buf, enc_ptr);
    }

    ret
}

pub fn sys_fcntl(fd: usize, cmd: usize, arg: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;
    const F_GETLK: usize = 5;
    const F_SETLK: usize = 6;
    const F_SETLKW: usize = 7;

    // flock not implement yet.
    if (arg == F_GETLK || arg == F_SETLK || arg == F_SETLKW) {
        return -1;
    }

    let args = ArgRegs {
        a0: fd,
        a1: cmd,
        a2: arg,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_FCNTL,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    ret
}

pub fn sys_getcwd(buf: usize, size: usize) -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let vstack_enc_addr = kernel.task.get_vstack_addr();
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - size;
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    if !check_shared_ptr_valid(size, pre_vstack.size()) {
        return -1;
    }

    let args = ArgRegs {
        a0: fixup_ptr,
        a1: size,
        a2: 0,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_GETCWD,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);
    // build buffer
    let rt_buf = UsrBuf::new(buf, size, Buf_Policy::Write, None);
    unsafe { copy_to_user(rt_buf, enc_ptr) };

    buf as isize
}

pub fn sys_ioctl(fd: usize, request: usize, arg: usize) -> isize {
    const TCGETS: usize = 0x5401;
    const TCSETS: usize = 0x5402;

    log::debug!("proxy ioctl: fd: {fd:#x}, request: {request:#x}");
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };

    // // create ipc channel and get the channel id
    // let channel_id = ipc_channel_open(arg, kernel.task.vmm.lock().gen_satp());
    // log::debug!("channel id: {channel_id:#x}");

    let vstack_enc_addr = kernel.task.get_vstack_addr();
    log::debug!("vstack enc addr: {:#x}", vstack_enc_addr);
    let pre_vstack = Vstack::from_addr(vstack_enc_addr);

    let enc_ptr = pre_vstack.sp() - pre_vstack.size();
    let offset = kernel.task.get_ht_offset();
    let fixup_ptr = enc_ptr + offset;

    let args = ArgRegs {
        a0: fd,
        a1: request,
        a2: arg as usize,
        a3: 0,
        a4: 0,
        a5: 0,
        a6: 0,
        a7: SYS_IOCTL,
    };

    unsafe { set_proxy_syscall_args(pre_vstack, args) };
    let ret = dispatch_proxy_syscall(vstack_enc_addr + offset, vstack_enc_addr);

    // // destroy the ipc channel by channel_id
    // ipc_channel_close(channel_id as usize);

    ret
}

pub fn ipc_channel_open(arg: usize, task_satp: usize) -> isize {
    let (error, channel_id) = sbi_open_channel(arg, task_satp);
    channel_id
}

pub fn ipc_channel_close(channel_id: usize) -> isize {
    let (error, value) = sbi_close_channel(channel_id);
    value
}

pub fn ipc_channel_recv(channel_id: usize) -> usize {
    let (error, arg) = sbi_recv_channel(channel_id);
    arg as usize
}

fn dispatch_proxy_syscall(vstack_host_addr: usize, vstack_enc_addr: usize) -> isize {
    let (error, value) = sbi_stop_enclave(vstack_host_addr);
    let vstack = Vstack::from_addr(vstack_enc_addr);
    let a0 = vstack.regs.a0;
    a0 as isize
}

unsafe fn set_proxy_syscall_args(vstack: &mut Vstack, args: ArgRegs) {
    vstack.regs = args;
}

fn check_shared_ptr_valid(len: usize, vstack_size: usize) -> bool {
    if (len > vstack_size) {
        return false;
    }
    return true;
}
