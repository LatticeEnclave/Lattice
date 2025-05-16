#![no_std]
#![feature(naked_functions)]

pub mod ecall;
mod macros;
mod scratch;
pub mod trap;
pub mod profiling;

pub use scratch::Scratch;
pub use trap::{TrapRegs, TrapRegsSMode};
