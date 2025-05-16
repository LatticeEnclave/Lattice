use crate::{kernel::LinuxUserKernel, usr::{Buf_Policy, UsrBuf, UsrOutPtr, UsrPtr}};
use riscv::register::time;

/// TimeSpec struct for clock_gettime, similar to Timespec
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct TimeSpec {
    /// seconds
    pub sec: usize,
    /// nano seconds
    pub nsec: usize,
}

/// TimeVal struct for gettimeofday
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct TimeVal {
    /// seconds
    pub sec: usize,
    /// microsecond
    pub usec: usize,
}

impl TimeSpec {
    pub fn new(sec: usize, nsec: usize) -> Self {
        Self {
            sec,
            nsec,
        }
    }
}

impl TimeVal {
    pub fn new(sec: usize, usec: usize) -> Self {
        Self {
            sec,
            usec,
        }
    }
}

pub fn sys_clock_gettime(clock: usize, buf: usize) -> isize {
    if buf == 0 {
        return -22;
    }

    let freq = unsafe { LinuxUserKernel::from_sscratch().device.cpu.time_freq };
    // build buffer
    let rt_buf = UsrBuf::new(buf, core::mem::size_of::<TimeSpec>(), Buf_Policy::Write, None);
    rt_buf.build_buf();
    // todo: the clock not truly implemented
    let t = get_time();
    let sec = t / freq;
    let nsec = (t % freq) * (1_000_000_000 / freq);
    let ts = TimeSpec::new(sec, nsec);

    let mut p :UsrOutPtr<TimeSpec> = UsrOutPtr::from_usr_buf(&rt_buf);
    let _ = p.write(ts);

    //retract the buffer
    rt_buf.retract_buf();

    0
}

pub fn sys_gettimeofday(buf: usize, tz: usize) -> isize {
    if tz != 0 {
        return -1;
    }

    let freq = unsafe { LinuxUserKernel::from_sscratch().device.cpu.time_freq };
    // build buffer
    let rt_buf = UsrBuf::new(buf, core::mem::size_of::<TimeVal>(), Buf_Policy::Write, None);
    rt_buf.build_buf();
    // todo: the clock not truly implemented
    let t = get_time();
    let sec = t / freq;
    let usec = (t % freq) / 1000;
    let tv = TimeVal::new(sec, usec);

    let mut p :UsrOutPtr<TimeVal> = UsrOutPtr::from_usr_buf(&rt_buf);
    let _ = p.write(tv);

    //retract the buffer
    rt_buf.retract_buf();

    0    
}

fn get_time() -> usize {
    time::read()
}