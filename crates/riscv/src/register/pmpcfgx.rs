//! Physical memory protection configuration

/// Permission enum contains all possible permission modes for pmp registers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    NONE = 0b000,
    R = 0b001,
    W = 0b010,
    RW = 0b011,
    X = 0b100,
    RX = 0b101,
    WX = 0b110,
    RWX = 0b111,
}

/// Range enum contains all possible addressing modes for pmp registers
#[derive(Clone, Copy, Debug)]
pub enum Range {
    OFF = 0b00,
    TOR = 0b01,
    NA4 = 0b10,
    NAPOT = 0b11,
}

/// Pmp struct holds a high-level representation of a single pmp configuration
#[derive(Clone, Copy, Debug)]
pub struct Pmp {
    /// raw bits
    pub byte: u8,
    /// Current PMP Permission
    pub permission: Permission,
    /// Current PMP Range
    pub range: Range,
    /// Is PMP locked?
    pub locked: bool,
}

pub struct Pmpcsr {
    /// Holds the raw contents of a PMP CSR Register
    pub bits: usize,
}

impl Pmpcsr {
    /// Take the register contents and translate into a Pmp configuration struct
    #[inline]
    pub fn into_config(&self, index: usize) -> Pmp {
        #[cfg(riscv32)]
        assert!(index < 4);

        #[cfg(riscv64)]
        assert!(index < 8);

        let byte = (self.bits >> (8 * index)) as u8; // move config to LSB and drop the rest
        let permission = byte & 0x7; // bits 0-2
        let range = (byte >> 3) & 0x3; // bits 3-4
        Pmp {
            byte,
            permission: match permission {
                0 => Permission::NONE,
                1 => Permission::R,
                2 => Permission::W,
                3 => Permission::RW,
                4 => Permission::X,
                5 => Permission::RX,
                6 => Permission::WX,
                7 => Permission::RWX,
                _ => unreachable!(),
            },
            range: match range {
                0 => Range::OFF,
                1 => Range::TOR,
                2 => Range::NA4,
                3 => Range::NAPOT,
                _ => unreachable!(),
            },
            locked: (byte & (1 << 7)) != 0,
        }
    }
}

pub mod pmpcfg {
    use super::*;

    #[inline]
    pub fn read(i: usize) -> super::Pmp {
        if i < 8 {
            pmpcfg0::read().into_config(i)
        } else if i < 16 {
            pmpcfg2::read().into_config(i - 8)
        } else if i < 24 {
            pmpcfg4::read().into_config(i - 16)
        } else if i < 32 {
            pmpcfg6::read().into_config(i - 24)
        } else {
            panic!()
        }
    }

    #[inline]
    pub unsafe fn set(i: usize, range: Range, perm: Permission, locked: bool) {
        unsafe {
            if i < 8 {
                pmpcfg0::set_pmp(i, range, perm, locked)
            } else if i < 16 {
                pmpcfg2::set_pmp(i - 8, range, perm, locked)
            } else if i < 24 {
                pmpcfg4::set_pmp(i - 16, range, perm, locked)
            } else if i < 32 {
                pmpcfg6::set_pmp(i - 24, range, perm, locked)
            } else {
                panic!()
            }
        }
    }
}

pub mod pmpentry {
    use crate::asm::*;
    use crate::register::*;

    use super::*;

    pub unsafe fn set(
        i: usize,
        range: Range,
        permission: Permission,
        locked: bool,
        addr_bits: usize,
    ) {
        unsafe {
            match i {
                0 => {
                    pmpcfg0::set_pmp(0, range, permission, locked);
                    pmpaddr0::write(addr_bits);
                }
                1 => {
                    pmpcfg0::set_pmp(1, range, permission, locked);
                    pmpaddr1::write(addr_bits);
                }
                2 => {
                    pmpcfg0::set_pmp(2, range, permission, locked);
                    pmpaddr2::write(addr_bits);
                }
                3 => {
                    pmpcfg0::set_pmp(3, range, permission, locked);
                    pmpaddr3::write(addr_bits);
                }
                4 => {
                    pmpcfg0::set_pmp(4, range, permission, locked);
                    pmpaddr4::write(addr_bits);
                }
                5 => {
                    pmpcfg0::set_pmp(5, range, permission, locked);
                    pmpaddr5::write(addr_bits);
                }
                6 => {
                    pmpcfg0::set_pmp(6, range, permission, locked);
                    pmpaddr6::write(addr_bits);
                }
                7 => {
                    pmpcfg0::set_pmp(7, range, permission, locked);
                    pmpaddr7::write(addr_bits);
                }
                8 => {
                    pmpcfg2::set_pmp(0, range, permission, locked);
                    pmpaddr8::write(addr_bits);
                }
                9 => {
                    pmpcfg2::set_pmp(1, range, permission, locked);
                    pmpaddr9::write(addr_bits);
                }
                10 => {
                    pmpcfg2::set_pmp(2, range, permission, locked);
                    pmpaddr10::write(addr_bits);
                }
                11 => {
                    pmpcfg2::set_pmp(3, range, permission, locked);
                    pmpaddr11::write(addr_bits);
                }
                12 => {
                    pmpcfg2::set_pmp(4, range, permission, locked);
                    pmpaddr12::write(addr_bits);
                }
                13 => {
                    pmpcfg2::set_pmp(5, range, permission, locked);
                    pmpaddr13::write(addr_bits);
                }
                14 => {
                    pmpcfg2::set_pmp(6, range, permission, locked);
                    pmpaddr14::write(addr_bits);
                }
                15 => {
                    pmpcfg2::set_pmp(7, range, permission, locked);
                    pmpaddr15::write(addr_bits);
                }
                16 => {
                    pmpcfg4::set_pmp(0, range, permission, locked);
                    pmpaddr16::write(addr_bits);
                }
                17 => {
                    pmpcfg4::set_pmp(1, range, permission, locked);
                    pmpaddr17::write(addr_bits);
                }
                18 => {
                    pmpcfg4::set_pmp(2, range, permission, locked);
                    pmpaddr18::write(addr_bits);
                }
                19 => {
                    pmpcfg4::set_pmp(3, range, permission, locked);
                    pmpaddr19::write(addr_bits);
                }
                20 => {
                    pmpcfg4::set_pmp(4, range, permission, locked);
                    pmpaddr20::write(addr_bits);
                }
                21 => {
                    pmpcfg4::set_pmp(5, range, permission, locked);
                    pmpaddr21::write(addr_bits);
                }
                22 => {
                    pmpcfg4::set_pmp(6, range, permission, locked);
                    pmpaddr22::write(addr_bits);
                }
                23 => {
                    pmpcfg4::set_pmp(7, range, permission, locked);
                    pmpaddr23::write(addr_bits);
                }
                24 => {
                    pmpcfg6::set_pmp(0, range, permission, locked);
                    pmpaddr24::write(addr_bits);
                }
                25 => {
                    pmpcfg6::set_pmp(1, range, permission, locked);
                    pmpaddr25::write(addr_bits);
                }
                26 => {
                    pmpcfg6::set_pmp(2, range, permission, locked);
                    pmpaddr26::write(addr_bits);
                }
                27 => {
                    pmpcfg6::set_pmp(3, range, permission, locked);
                    pmpaddr27::write(addr_bits);
                }
                28 => {
                    pmpcfg6::set_pmp(4, range, permission, locked);
                    pmpaddr28::write(addr_bits);
                }
                29 => {
                    pmpcfg6::set_pmp(5, range, permission, locked);
                    pmpaddr29::write(addr_bits);
                }
                30 => {
                    pmpcfg6::set_pmp(6, range, permission, locked);
                    pmpaddr30::write(addr_bits);
                }
                31 => {
                    pmpcfg6::set_pmp(7, range, permission, locked);
                    pmpaddr31::write(addr_bits);
                }
                _ => panic!(),
            }
        }
        sfence_vma_all();
        fence();
    }
}

/// Physical memory protection configuration
/// pmpcfg0 struct contains pmp0cfg - pmp3cfg for RV32, and pmp0cfg - pmp7cfg for RV64
pub mod pmpcfg0 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A0);
    write_csr_as_usize!(0x3A0);

    set_pmp!();
    clear_pmp!();
}

/// Physical memory protection configuration
/// pmpcfg1 struct contains pmp4cfg - pmp7cfg for RV32 only
#[cfg(riscv32)]
pub mod pmpcfg1 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A1);
    write_csr_as_usize_rv32!(0x3A1);

    set_pmp!();
    clear_pmp!();
}

/// Physical memory protection configuration
/// pmpcfg2 struct contains pmp8cfg - pmp11cfg for RV32, or pmp8cfg - pmp15cfg for RV64
pub mod pmpcfg2 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A2);
    write_csr_as_usize!(0x3A2);

    set_pmp!();
    clear_pmp!();
}

/// Physical memory protection configuration
/// pmpcfg3 struct contains pmp12cfg - pmp15cfg for RV32 only
#[cfg(riscv32)]
pub mod pmpcfg3 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A3);
    write_csr_as_usize_rv32!(0x3A3);

    set_pmp!();
    clear_pmp!();
}

pub mod pmpcfg4 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A4);
    write_csr_as_usize!(0x3A4);

    set_pmp!();
    clear_pmp!();
}

pub mod pmpcfg6 {
    use super::{Permission, Pmpcsr, Range};

    read_csr_as!(Pmpcsr, 0x3A6);
    write_csr_as_usize!(0x3A6);

    set_pmp!();
    clear_pmp!();
}
