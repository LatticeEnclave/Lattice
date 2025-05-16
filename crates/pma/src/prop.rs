use core::fmt::Display;

use bit_field::BitField;
use riscv::register::Permission;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Owner(pub usize);

impl Owner {
    pub const EVERYONE: Self = Self(0);
    pub const HOST: Self = Self(1);
    pub const START: Self = Self(2);
}

impl From<usize> for Owner {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl Into<usize> for Owner {
    fn into(self) -> usize {
        self.0
    }
}

impl Owner {
    pub fn new() -> Self {
        Self::START
    }
}

impl Display for Owner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 描述了一块PMA区域的所有者以及所有者拥有的权限
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PmaProp {
    bits: usize,
}

impl PmaProp {
    #[inline(always)]
    pub fn empty() -> Self {
        Self { bits: 0 }
    }

    #[inline(always)]
    pub fn bits(&self) -> usize {
        self.bits
    }

    #[inline(always)]
    pub fn owner(mut self, owner: impl Into<Owner>) -> Self {
        self.bits.set_bits(3..64, owner.into().0);
        self
    }

    #[inline(always)]
    pub fn permission(mut self, permission: impl Into<Permission>) -> Self {
        let perm: Permission = permission.into();
        let bits = match perm {
            Permission::NONE => 0b000,
            Permission::R => 0b001,
            Permission::W => 0b010,
            Permission::RW => 0b011,
            Permission::X => 0b100,
            Permission::RX => 0b101,
            Permission::WX => 0b110,
            Permission::RWX => 0b111,
        };
        self.bits.set_bits(0..=2, bits);
        self
    }

    #[inline(always)]
    pub fn get_owner(&self) -> Owner {
        Owner(self.bits.get_bits(3..64))
    }

    #[inline(always)]
    pub fn get_owner_perm(&self) -> Permission {
        match self.bits.get_bits(0..=2) {
            0 => Permission::NONE,
            1 => Permission::R,
            2 => Permission::W,
            3 => Permission::RW,
            4 => Permission::X,
            5 => Permission::RX,
            6 => Permission::WX,
            7 => Permission::RWX,
            _ => unreachable!(),
        }
    }
}

impl Default for PmaProp {
    fn default() -> Self {
        Self::empty().permission(Permission::RWX).owner(Owner::HOST)
    }
}
