use core::ops::Add;

use crate::usr::{Buf_Policy, UsrBuf, UsrOutPtr, UsrPtr};

use rand_core::{RngCore, SeedableRng};

/// pedo-random number generator
/// Xorshift32
pub struct Xorshift32 {
    state: u32,
}

impl SeedableRng for Xorshift32 {
    type Seed = [u8; 4];

    fn from_seed(seed: Self::Seed) -> Xorshift32 {
        let mut state = 0u32;
        state |= (seed[0] as u32) << 24;
        state |= (seed[1] as u32) << 16;
        state |= (seed[2] as u32) << 8;
        state |= seed[3] as u32;
        Xorshift32 { state }        
    }
}

impl RngCore for Xorshift32 {
    fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    fn next_u64(&mut self) -> u64 {
        let upper = self.next_u32() as u64;
        let lower = self.next_u32() as u64;
        (upper << 32) | lower
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut bytes = dest.chunks_exact_mut(4);
        for chunk in &mut bytes {
            let random = self.next_u32().to_ne_bytes();
            chunk.copy_from_slice(&random);
        }
        let remainder = bytes.into_remainder();
        let random = self.next_u32().to_ne_bytes();
        remainder.copy_from_slice(&random[..remainder.len()]);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

pub struct RandGenerator {
    xorshift32: Xorshift32
}

impl RandGenerator {
    pub fn new(seed: usize) -> Self {
        let seed_array = [(seed & 0xff) as u8, (seed & 0xff00) as u8, (seed & 0xff0000) as u8, (seed & 0xff000000) as u8];
        let rng = Xorshift32::from_seed(seed_array); 
        Self {
            xorshift32: rng,
        }       
    }

    pub fn next_u8(&mut self) -> u8 {
        self.xorshift32
            .next_u32() as u8
    }

    pub fn next_u32(&mut self) -> u32 {
        self.xorshift32
            .next_u32()
    }
    
    pub fn next_u64(&mut self) -> u64 {
        self.xorshift32
            .next_u64()
    }

 }

pub fn sys_getrandom(buf: usize, len: usize, flag: u32) -> isize {
    let ulen = core::mem::size_of::<usize>();
    if len % ulen != 0 {
        return 0;
    }
    let mut rand = RandGenerator::new(buf);
    // build buffer
    let rt_buf = UsrBuf::new(buf, len, Buf_Policy::Write, None);
    rt_buf.build_buf();

    let p: UsrOutPtr<u8> = UsrPtr::from_usr_buf(&rt_buf);
    for i in 0..len {
        let rand_num = rand.next_u8();
        let _ = p.add(i).write(rand_num);
    }

    rt_buf.retract_buf();

    len as isize
}

pub fn sys_uname(buf: usize) -> isize {
    const OFFSET: usize = 65;
    // build buffer
    let rt_buf = UsrBuf::new(buf, OFFSET * 5, Buf_Policy::Write, None);
    rt_buf.build_buf();

    let p: UsrOutPtr<u8> = UsrPtr::from_usr_buf(&rt_buf);    
    let strings = [
        "Linux",        //sysname
        "Encl",         //nodename
        "5.16",         //release
        "HTEE-OS",      //version
        "riscv64",      //machine
    ];

    for(i, &s) in strings.iter().enumerate() {
        let _ = p.add(i * OFFSET).write_cstring(s);
    }

    rt_buf.retract_buf();
    0
}