#![no_std]
#![feature(str_from_raw_parts)]

pub mod call;
pub mod channel;
pub mod enclave;
pub mod info;
pub mod op;
pub mod proxy;

pub mod h2e {
    pub use crate::enclave::client::*;
    pub use crate::info::*;
}

pub mod e2r {
    pub use crate::enclave::runtime::*;
}
