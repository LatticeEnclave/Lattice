#[allow(dead_code)]
pub struct RelocationEntry {
    pub offset: usize,
    pub symval: Option<usize>,
    pub addend: usize,
}