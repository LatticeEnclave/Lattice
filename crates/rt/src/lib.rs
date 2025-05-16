#![no_std]
#![feature(naked_functions)]
#![feature(offset_of)]
#![feature(allocator_api)]
#![feature(asm_const)]
#![feature(type_alias_impl_trait)]
#![feature(error_in_core)]

extern crate alloc;

pub mod consts;
pub mod context;
pub mod error;
mod frame;
mod heap;
pub mod kernel;
// mod mem;
pub mod console;
mod ldesym;
mod loader;
pub mod macros;
pub mod pt;
mod scratch;
mod stack;
pub mod syscall;
mod task;
pub mod trampoline;
pub mod trap;
mod usr;

pub use error::Error;
pub type Result<T> = core::result::Result<T, Error>;
pub use frame::PhysMemMgr;
pub use heap::{LdHeapAllocator, LuHeapAllocator};
// pub use mem::RtPtWriter;
pub use context::TrapRegsSMode;
pub use scratch::Scratch;

pub use spin::Mutex;

pub use console::_print;

pub mod log {
    pub use crate::{debug, error, info, trace, warn};
}
