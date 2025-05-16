use crate::kernel::{self, LinuxUserKernel};

pub fn sys_getpid() -> isize {
    let kernel = unsafe { LinuxUserKernel::from_sscratch() };
    let pid = kernel
                        .task
                        .get_pid();
    
    pid as isize
}