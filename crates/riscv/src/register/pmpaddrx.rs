macro_rules! reg {
    (
        $addr:expr, $csr:ident
    ) => {
        /// Physical memory protection address register
        pub mod $csr {
            read_csr_as_usize!($addr);
            write_csr_as_usize!($addr);
        }
    };
}

macro_rules! reg_group {
    (
        $base:expr
    ) => {
        
    };
}

pub mod pmpaddr {
    /// Read pmpaddr register
    #[inline]
    pub fn read(i: usize) -> usize {
        use super::*;

        match i {
            0x0 => pmpaddr0::read(),
            0x1 => pmpaddr1::read(),
            0x2 => pmpaddr2::read(),
            0x3 => pmpaddr3::read(),
            0x4 => pmpaddr4::read(),
            0x5 => pmpaddr5::read(),
            0x6 => pmpaddr6::read(),
            0x7 => pmpaddr7::read(),
            0x8 => pmpaddr8::read(),
            0x9 => pmpaddr9::read(),
            0xa => pmpaddr10::read(),
            0xb => pmpaddr11::read(),
            0xc => pmpaddr12::read(),
            0xd => pmpaddr13::read(),
            0xe => pmpaddr14::read(),
            0xf => pmpaddr15::read(),
            0x10 => pmpaddr16::read(),
            0x11 => pmpaddr17::read(),
            0x12 => pmpaddr18::read(),
            0x13 => pmpaddr19::read(),
            0x14 => pmpaddr20::read(),
            0x15 => pmpaddr21::read(),
            0x16 => pmpaddr22::read(),
            0x17 => pmpaddr23::read(),
            0x18 => pmpaddr24::read(),
            0x19 => pmpaddr25::read(),
            0x1a => pmpaddr26::read(),
            0x1b => pmpaddr27::read(),
            0x1c => pmpaddr28::read(),
            0x1d => pmpaddr29::read(),
            0x1e => pmpaddr30::read(),
            0x1f => pmpaddr31::read(),
            _ => todo!(),
        }
    }

    #[inline]
    pub fn set(i: usize, bits: usize) {
        use super::*;

        match i {
            0x0 => pmpaddr0::write(bits),
            0x1 => pmpaddr1::write(bits),
            0x2 => pmpaddr2::write(bits),
            0x3 => pmpaddr3::write(bits),
            0x4 => pmpaddr4::write(bits),
            0x5 => pmpaddr5::write(bits),
            0x6 => pmpaddr6::write(bits),
            0x7 => pmpaddr7::write(bits),
            0x8 => pmpaddr8::write(bits),
            0x9 => pmpaddr9::write(bits),
            0xa => pmpaddr10::write(bits),
            0xb => pmpaddr11::write(bits),
            0xc => pmpaddr12::write(bits),
            0xd => pmpaddr13::write(bits),
            0xe => pmpaddr14::write(bits),
            0xf => pmpaddr15::write(bits),
            0x10 => pmpaddr16::write(bits),
            0x11 => pmpaddr17::write(bits),
            0x12 => pmpaddr18::write(bits),
            0x13 => pmpaddr19::write(bits),
            0x14 => pmpaddr20::write(bits),
            0x15 => pmpaddr21::write(bits),
            0x16 => pmpaddr22::write(bits),
            0x17 => pmpaddr23::write(bits),
            0x18 => pmpaddr24::write(bits),
            0x19 => pmpaddr25::write(bits),
            0x1a => pmpaddr26::write(bits),
            0x1b => pmpaddr27::write(bits),
            0x1c => pmpaddr28::write(bits),
            0x1d => pmpaddr29::write(bits),
            0x1e => pmpaddr30::write(bits),
            0x1f => pmpaddr31::write(bits),
            _ => todo!(),
        }
    }
}

reg!(0x3B0, pmpaddr0);
reg!(0x3B1, pmpaddr1);
reg!(0x3B2, pmpaddr2);
reg!(0x3B3, pmpaddr3);
reg!(0x3B4, pmpaddr4);
reg!(0x3B5, pmpaddr5);
reg!(0x3B6, pmpaddr6);
reg!(0x3B7, pmpaddr7);
reg!(0x3B8, pmpaddr8);
reg!(0x3B9, pmpaddr9);
reg!(0x3BA, pmpaddr10);
reg!(0x3BB, pmpaddr11);
reg!(0x3BC, pmpaddr12);
reg!(0x3BD, pmpaddr13);
reg!(0x3BE, pmpaddr14);
reg!(0x3BF, pmpaddr15);
reg!(0x3C0, pmpaddr16);
reg!(0x3C1, pmpaddr17);
reg!(0x3C2, pmpaddr18);
reg!(0x3C3, pmpaddr19);
reg!(0x3C4, pmpaddr20);
reg!(0x3C5, pmpaddr21);
reg!(0x3C6, pmpaddr22);
reg!(0x3C7, pmpaddr23);
reg!(0x3C8, pmpaddr24);
reg!(0x3C9, pmpaddr25);
reg!(0x3CA, pmpaddr26);
reg!(0x3CB, pmpaddr27);
reg!(0x3CC, pmpaddr28);
reg!(0x3CD, pmpaddr29);
reg!(0x3CE, pmpaddr30);
reg!(0x3CF, pmpaddr31);
