mod file;
#[allow(unused)]
pub mod linux_wrap;
mod mem;
mod misc;
mod task;
mod time;
mod vm;

use file::{
    sys_close, sys_epoll_create1, sys_epoll_ctl, sys_epoll_pwait, sys_fcntl, sys_fstat, sys_fsync,
    sys_ftruncate, sys_getcwd, sys_ioctl, sys_lseek, sys_newfstatat, sys_openat, sys_pipe2,
    sys_read, sys_readv, sys_sync, sys_unlinkat, sys_write, sys_writev,
};
use linux_wrap::{
    SYS_BRK, SYS_CLOCK_GETTIME, SYS_CLOSE, SYS_EPOLL_CREATE1, SYS_EPOLL_CTL, SYS_EPOLL_PWAIT,
    SYS_EXIT, SYS_EXIT_GROUP, SYS_FCNTL, SYS_FSTAT, SYS_FSYNC, SYS_FTRUNCATE, SYS_FUTEX,
    SYS_GETCWD, SYS_GETPID, SYS_GETRANDOM, SYS_GETTIMEOFDAY, SYS_IOCTL, SYS_LSEEK, SYS_MMAP,
    SYS_MPROTECT, SYS_MUNMAP, SYS_NEWFSTATAT, SYS_OPENAT, SYS_PIPE2, SYS_READ, SYS_READV,
    SYS_RT_SIGACTION, SYS_RT_SIGPROCMASK, SYS_SET_ROBUST_LIST, SYS_SET_TID_ADDRESS, SYS_SYNC,
    SYS_UNAME, SYS_UNLINKAT, SYS_WRITE, SYS_WRITEV,
};
use mem::{sys_brk, sys_mmap, sys_mprotect, sys_munmap};
pub use misc::RandGenerator;
use misc::{sys_getrandom, sys_uname};
use sbi::ecall::{
    sbi_call_1, sbi_unimp_1, sbi_unimp_2, sbi_unimp_3, SBISMEnclaveCall, SBI_EXT_TEE_ENCLAVE,
};
use task::sys_getpid;
use time::{sys_clock_gettime, sys_gettimeofday};

use crate::log;

pub fn sbi_exit_enclave(retval: usize) -> (isize, isize) {
    sbi_call_1(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMExitEnclave as usize,
        retval,
    )
}

pub fn sbi_stop_enclave(request: usize) -> (isize, isize) {
    sbi_call_1(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMStopEnclave as usize,
        request,
    )
}

pub fn sbi_open_channel(request1: usize, request2: usize) -> (isize, isize) {
    sbi_unimp_2(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMChannelOpen as usize,
        request1,
        request2,
    )
}

pub fn sbi_close_channel(request: usize) -> (isize, isize) {
    sbi_unimp_1(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMChannelClose as usize,
        request,
    )
}

pub fn sbi_recv_channel(request: usize) -> (isize, isize) {
    sbi_unimp_1(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMChannelConnect as usize,
        request,
    )
}

pub fn sbi_copy_from_user(to: usize, from: usize, size: usize) -> (isize, isize) {
    sbi_unimp_3(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMCopyFromLue as usize,
        to,
        from,
        size,
    )
}

pub fn sbi_copy_to_user(to: usize, from: usize, size: usize) -> (isize, isize) {
    sbi_unimp_3(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMCopyToLue as usize,
        to,
        from,
        size,
    )
}

pub fn sbi_copy_from_kernel(to: usize, from: usize, size: usize) -> (isize, isize) {
    sbi_unimp_3(
        SBI_EXT_TEE_ENCLAVE,
        SBISMEnclaveCall::SbiSMCopyFromKernel as usize,
        to,
        from,
        size,
    )
}

pub fn linux_syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    log::debug!("linux syscall: {:#x}", syscall_id);
    let [a0, a1, a2, a3, a4, a5] = args;
    let ret: isize = match syscall_id {
        // linux system cmd
        SYS_CLOCK_GETTIME => sys_clock_gettime(a0, a1.into()),
        SYS_GETTIMEOFDAY => sys_gettimeofday(a0, a1),
        SYS_GETRANDOM => sys_getrandom(a0.into(), a1, a2 as u32),
        SYS_RT_SIGPROCMASK => sys_unimplemented("rt_sigprocmask", 0),
        SYS_GETPID => sys_getpid(),
        SYS_UNAME => sys_uname(a0.into()),
        SYS_RT_SIGACTION => sys_unimplemented("rt_sigaction", 0),
        SYS_SET_TID_ADDRESS => sys_unimplemented("set_tid_address", 1),
        SYS_SET_ROBUST_LIST => sys_unimplemented("set_robust_list", 0),
        SYS_BRK => sys_brk(a0.into()),
        SYS_MMAP => sys_mmap(a0, a1, a2, a3, a4 as isize, a5 as _),
        SYS_MUNMAP => sys_munmap(a0, a1),
        SYS_MPROTECT => sys_mprotect(a0, a1, a2),
        SYS_EXIT | SYS_EXIT_GROUP => {
            sbi_exit_enclave(a0);
            0
        }
        SYS_FUTEX => sys_unimplemented("futex", 0),

        // file operations
        SYS_READ => sys_read(a0, a1, a2),
        SYS_WRITE => sys_write(a0, a1, a2),
        SYS_READV => sys_readv(a0, a1, a2),
        SYS_WRITEV => sys_writev(a0, a1, a2),
        SYS_OPENAT => sys_openat(a0, a1, a2 as isize, a3),
        SYS_UNLINKAT => sys_unlinkat(a0, a1, a2),
        SYS_NEWFSTATAT => sys_newfstatat(a0, a1, a2, a3),
        SYS_PIPE2 => sys_pipe2(a0),
        SYS_LSEEK => sys_lseek(a0, a1, a2),
        SYS_FTRUNCATE => sys_ftruncate(a0, a1),
        SYS_SYNC => sys_sync(),
        SYS_FSYNC => sys_fsync(a0),
        SYS_CLOSE => sys_close(a0),
        SYS_EPOLL_CREATE1 => sys_epoll_create1(a0),
        SYS_EPOLL_CTL => sys_epoll_ctl(a0, a1, a2, a3),
        SYS_EPOLL_PWAIT => sys_epoll_pwait(a0, a1, a2, a3),
        SYS_FCNTL => sys_fcntl(a0, a1, a2),
        SYS_GETCWD => sys_getcwd(a0, a1),
        SYS_FSTAT => sys_fstat(a0, a1),
        SYS_IOCTL => sys_ioctl(a0, a1, a2),

        _ => -1,
    };
    ret
}

/// unimplemented syscalls
fn sys_unimplemented(name: &str, ret: isize) -> isize {
    ret
}
