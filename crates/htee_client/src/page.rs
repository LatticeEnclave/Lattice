#[derive(Clone)]
#[repr(align(0x1000))]
pub struct Page([u8; 0x1000]);

impl Page {
    pub fn new() -> Self {
        Self([0; 0x1000])
    }

    pub fn set_value(&mut self, value: usize) {
        let bytes = value.to_be_bytes();

        self.0[0..8].clone_from_slice(&bytes);
    }

    fn print_8_byte(&self) {
        self.0[0..8].iter().for_each(|b| print!("{b:#x}, "));
        println!("")
    }

    pub fn addr(&self) -> usize {
        self as *const Self as usize
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

impl Into<Vec<u8>> for Page {
    fn into(self) -> Vec<u8> {
        self.0.into()
    }
}

pub const HUGE_PAGE_SIZE: usize = 2 * 1024 * 1024;

#[repr(align(0x200000))]
pub struct HugePage {}
