use crate::console::Uart;

#[derive(Default)]
pub struct Mmio {
    pub uart: Option<Uart>,
    pub dma: usize,
}

impl Mmio {
    pub fn get_dma(&self) -> Option<usize> {
        if self.dma == 0 { None } else { Some(self.dma) }
    }
}
