use libc::*;
use std::{fs::File, io, os::fd::AsRawFd, ptr};

pub type Error = i32;

#[repr(C)]
pub struct UserArg {
    pub size: usize,
    pub fd: i32,
}

const TEECTL_IOCTL_ALLOC: usize = 0x40106b01;
const TEECTL_IOCTL_FREE: usize = 0x40106b02;

const TEECTL_DEV: &str = "/dev/teectl";

static mut MEM_FD: i32 = 0;

pub fn teectl_open() -> File {
    let file = File::open(TEECTL_DEV).expect("Failed to open device");
    file
}

fn teectl_ioctl(command: usize, argp: &mut UserArg) -> Result<(), io::Error> {
    let f = teectl_open();
    println!("fd: {}", f.as_raw_fd());
    println!("command: {:#x}", command);
    let error = unsafe { ioctl(f.as_raw_fd(), command as u64, argp) };
    if error < 0 {
        return Err(io::Error::last_os_error());
    }
    // teectl_close(fd);
    Ok(())
}

pub fn teectl_alloc(size: usize) -> Result<usize, Error> {
    let mut user_arg = Box::new(UserArg { size, fd: -1 });
    teectl_ioctl(TEECTL_IOCTL_ALLOC, user_arg.as_mut()).unwrap();
    unsafe {
        let mmap_fd = user_arg.fd;
        MEM_FD = mmap_fd;

        println!("mmap fd: {}", mmap_fd);
        println!("size: {}", user_arg.size);
        let addr = mmap(
            ptr::null_mut(),
            user_arg.size,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            user_arg.fd,
            0,
        );

        if addr == MAP_FAILED {
            panic!("mmap failed");
        }

        println!("mmap addr: {:p}", addr);

        Ok(addr as usize)
    }
}

pub fn teectl_free(addr: usize, size: usize) -> Result<(), io::Error> {
    unsafe {
        let ret = munmap(addr as *mut _, size);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut user_arg = Box::new(UserArg {
        size: 0,
        fd: unsafe { MEM_FD },
    });
    teectl_ioctl(TEECTL_IOCTL_FREE, user_arg.as_mut()).unwrap();
    Ok(())
}
