pub struct Stack<const SIZE: usize>([u8; SIZE]);

impl<const SIZE: usize> Stack<SIZE> {
    pub fn new() -> Self {
        Self([0; SIZE])
    }
//简易的栈
    pub fn top(&self) -> usize {
        self.0.as_ptr() as usize + SIZE
    }
}

/// usr env stack struct
pub struct StackEnv {
    pub argc: usize,
    pub argv: usize,
    pub envp: usize,
    pub hwcap_key: usize,
    pub hwcap_val: usize,
    // pub sysinfo_key: usize,
    // pub sysinfo_val: usize,
    pub pagesz_key: usize,
    pub pagesz_val: usize,
    pub execfn_key: usize,
    pub execfn_val: usize,
    pub sec_key: usize,
    pub sec_val: usize,
    pub rand_key: usize,
    pub rand_val: usize,
    pub gid_key: usize,
    pub gid_val: usize,
    pub egid_key: usize,
    pub egid_val: usize,
    pub uid_key: usize,
    pub uid_val: usize,
    pub euid_key: usize,
    pub euid_val: usize,
    pub phdr_key: usize,
    pub phdr_val: usize,
    pub phnum_key: usize,
    pub phnum_val: usize,
    pub null_key: usize,
    pub null_val: usize,
    pub rand_num1: usize,
    pub rand_num2: usize,
}

