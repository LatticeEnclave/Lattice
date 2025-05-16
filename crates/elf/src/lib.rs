#![no_std]

#[derive(Debug, Clone)]
pub struct Sections {
    pub text: usize,
    pub text_unlikely: usize,
}
