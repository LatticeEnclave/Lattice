#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpCode(u64);

impl OpCode {
    pub const READ: OpCode = OpCode(0);
    pub const WRITE: OpCode = OpCode(1);
    pub const FINISH_READ: OpCode = OpCode(2);
    pub const FINISH_WRITE: OpCode = OpCode(3);
}

#[repr(C, align(0x1000))]
pub struct Head {
    pub id: u64,
    pub op_code: OpCode,
    pub arg0: u64,
    pub arg1: u64,
    pub stash: u64,
    pub len: u64,
    pub paddr: [u64; (0x1000 - 6 * size_of::<u64>()) / size_of::<u64>()],
}

impl Head {
    pub fn from_ptr(ptr: *const Head) -> &'static mut Self {
        assert!(ptr.is_aligned());
        unsafe { &mut *(ptr as *mut Head) }
    }

    pub fn id(&mut self, id: u64) -> &mut Self {
        self.id = id;
        self
    }

    pub fn op_code(&mut self, op_code: OpCode) -> &mut Self {
        self.op_code = op_code;
        self
    }

    pub fn arg0(&mut self, arg0: u64) -> &mut Self {
        self.arg0 = arg0;
        self
    }

    pub fn arg1(&mut self, arg1: u64) -> &mut Self {
        self.arg1 = arg1;
        self
    }

    pub fn stash(&mut self, stash: u64) -> &mut Self {
        self.stash = stash;
        self
    }

    pub fn push_paddr(&mut self, paddr: u64) -> &mut Self {
        if self.len as usize >= self.paddr.len() {
            panic!("paddr buffer is full");
        }
        self.paddr[self.len as usize] = paddr;
        self.len += 1;
        self
    }

    pub fn iter_paddr(&self) -> impl Iterator<Item = u64> + use<'_> {
        self.paddr[0..self.len as usize].iter().map(|v| *v)
    }
}
