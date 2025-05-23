#![no_std]

use core::{fmt, ops::Range};
use heapless::Vec;
use htee_console::log;
use htee_macro::usize_env_or;
use riscv::register::{Permission, pmpaddr, pmpcfg, pmpentry};

use pma::PhysMemArea;

mod cache;
pub use {cache::Cache, cache::NwCache, cache::NwCacheExt};

pub const MAX_PMP_COUNT: usize = 32;
pub const PMP_COUNT: usize = usize_env_or!("PMP_COUNT", 16);
// // pub const PMP_INIT_COUNT: usize = 16;
// pub const PMP_GRAN: usize = usize_env_or!("PMP_GRAN", 11);
pub const PMP_ADDR_BITS: usize = usize_env_or!("PMP_ADDR_BITS", 38);

pub type Mode = riscv::register::Range;

pub type PmpBuf = Vec<PmpHelper, MAX_PMP_COUNT>;

#[derive(Debug, Clone)]
pub struct PmpHelper {
    pub addr: usize,
    pub pma: PhysMemArea,
    pub is_tor: bool,
}

#[inline]
pub fn reset_pmp_registers() {
    unsafe {
        flush_pmp([].into_iter());
        PmpStatus::from_register(0)
            .region(0..0x1000)
            .napot()
            .permission(Permission::NONE)
            .apply(0);
    }
}

#[inline]
pub fn iter_hps() -> impl Iterator<Item = PmpStatus> {
    (0..PMP_COUNT).map(|idx| PmpStatus::from_register(idx))
}

pub fn hps_from_regs() -> Vec<PmpStatus, PMP_COUNT> {
    let mut hps = Vec::new();
    let mut prev_pmp: Option<PmpStatus> = None;
    for i in 0..PMP_COUNT {
        let s = PmpStatus::from_register(i);

        if i > 0 && s.is_tor() {
            if let Some(prev) = prev_pmp {
                if prev.is_off() {
                    hps.push(prev).unwrap();
                }
            }
        }

        if !s.is_off() {
            hps.push(s.clone()).unwrap();
        }

        prev_pmp = Some(s.clone());
    }

    hps
}

pub unsafe fn flush_pmp(pmps: impl Iterator<Item = PmpStatus>) {
    let mut idx = 0;

    for mut pmp in pmps {
        log::trace!("flushing {} , ty: {}", pmp, pmp.mode as usize);
        match pmp.get_mode() {
            Mode::NAPOT => unsafe { pmp.napot().apply(idx) },
            Mode::OFF => unsafe { pmp.off().apply(idx) },
            Mode::TOR => unsafe {
                pmp.off().apply(idx);
                pmp.tor().apply(idx + 1);
                idx += 1;
            },
            _ => panic!("Unsupported PMP type"),
        }
        idx += 1;
    }

    // turn the remaining pmp off
    for i in idx..PMP_COUNT {
        let mut status = PmpStatus::from_register(i);
        if !status.is_off() {
            unsafe { status.off().apply(i) }
        }
    }
}

#[derive(Debug)]
pub struct PmpRegGroup(Vec<PmpStatus, PMP_COUNT>);

impl PmpRegGroup {
    #[inline]
    pub fn from_registers() -> Self {
        let mut status = Vec::new();
        for i in 0..PMP_COUNT {
            let s = PmpStatus::from_register(i);
            status.push(s).unwrap();
        }

        Self(status)
    }

    #[inline]
    pub fn print(&self) {
        for (idx, i) in self.0.iter().enumerate() {
            log::debug!("{idx:>2}. {i}");
        }
    }

    #[inline]
    pub unsafe fn flush(&self) {
        for (i, entry) in self.0.iter().enumerate() {
            unsafe { entry.apply(i) };
        }
    }
}

impl Default for PmpRegGroup {
    fn default() -> Self {
        let mut status = Vec::new();
        let mut pmp = PmpStatus::new();
        pmp.region(0x0000_0000..0x0000_1000).napot();
        status.push(pmp).unwrap();

        Self(status)
    }
}

#[inline]
fn parser_region(mode: Mode, addr: usize, prev_addr: usize) -> Range<usize> {
    let mut region = 0..0;
    match mode {
        Mode::OFF => {
            region.start = (addr.clone()) << 2;
            region.end = (addr.clone()) << 2;
        }
        Mode::TOR => {
            region.start = prev_addr << 2;
            region.end = addr << 2;
        }
        Mode::NA4 => {
            region = parser_na4(addr);
        }
        Mode::NAPOT => {
            region = parser_napot(addr);
        }
    }

    region
}

fn parser_napot(addr: usize) -> Range<usize> {
    const PADDING: usize = (1 << (11 - 2)) - 1;

    let addr = addr | PADDING;
    let len = addr.trailing_ones();
    if len >= PMP_ADDR_BITS as u32 {
        0..usize::MAX
    } else {
        let size = 1usize << addr.trailing_ones() << 3;
        let start = (((1usize << addr.trailing_ones()) - 1) ^ addr) << 2;
        start..(start + size)
    }
}

fn parser_na4(addr: usize) -> Range<usize> {
    let size = 4;
    let start = addr << 2;

    start..(start + size)
}

#[derive(Clone, Debug)]
pub struct PmpStatus {
    pub region: Range<usize>,
    pub mode: Mode,
    pub permission: Permission,
    pub locked: bool,
}

impl PmpStatus {
    pub const fn new() -> Self {
        Self {
            region: 0..0,
            mode: Mode::OFF,
            permission: Permission::NONE,
            locked: false,
        }
    }

    pub fn from_pma(pma: PhysMemArea, mode: Mode) -> Self {
        Self {
            region: pma.region,
            mode,
            permission: pma.prop.get_owner_perm(),
            locked: false,
        }
    }

    pub fn get_mode(&self) -> Mode {
        self.mode
    }

    pub fn from_register(idx: usize) -> Self {
        let cfg = pmpcfg::read(idx);
        let addr = pmpaddr::read(idx);
        let region = match cfg.range {
            Mode::TOR => {
                let prev_addr = idx.checked_sub(1).map(pmpaddr::read).unwrap_or(0);
                parser_region(Mode::TOR, addr, prev_addr)
            }
            _ => parser_region(cfg.range, addr, 0),
        };
        Self {
            region,
            mode: cfg.range,
            permission: cfg.permission,
            locked: cfg.locked,
        }
    }

    pub fn get_region(&self) -> Range<usize> {
        self.region.clone()
    }

    pub fn is_tor(&self) -> bool {
        self.mode as usize == Mode::TOR as usize
    }

    pub fn is_off(&self) -> bool {
        self.mode as usize == Mode::OFF as usize
    }

    pub fn is_napot(&self) -> bool {
        self.mode as usize == Mode::NAPOT as usize
    }

    #[inline(always)]
    pub fn region(&mut self, region: Range<usize>) -> &mut Self {
        self.region = region;
        self
    }

    #[inline(always)]
    pub fn permission(&mut self, permission: Permission) -> &mut Self {
        self.permission = permission;
        self
    }

    #[inline(always)]
    pub fn off(&mut self) -> &mut Self {
        self.mode = Mode::OFF;
        self
    }

    #[inline(always)]
    pub fn napot(&mut self) -> &mut Self {
        self.mode = Mode::NAPOT;
        self
    }

    #[inline(always)]
    pub fn tor(&mut self) -> &mut Self {
        self.mode = Mode::TOR;
        self
    }

    #[inline(always)]
    pub fn contains(&self, addr: usize) -> bool {
        self.region.contains(&addr)
    }

    #[inline(never)]
    pub unsafe fn apply(&self, idx: usize) {
        let addr_bits: usize;

        match self.mode {
            Mode::NAPOT => {
                addr_bits =
                    (self.region.start >> 2) + ((self.region.end - self.region.start) >> 3) - 1;
            }
            Mode::OFF => {
                addr_bits = self.region.start >> 2;
            }
            Mode::TOR => {
                addr_bits = self.region.end >> 2;
            }
            _ => {
                panic!()
            }
        }

        unsafe {
            pmpentry::set(idx, self.mode, self.permission, false, addr_bits);
        };
    }
}

impl fmt::Display for PmpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let start = self.region.start;
        let end = self.region.end;
        f.write_fmt(format_args!(
            "{}{:#010x}-{:#010x} ({}{}{}{})",
            if let Mode::OFF = self.mode { " " } else { "*" },
            start,
            end,
            if self.permission as usize & 0b100 != 0 {
                "x"
            } else {
                "-"
            },
            if self.permission as usize & 0b010 != 0 {
                "w"
            } else {
                "-"
            },
            if self.permission as usize & 0b001 != 0 {
                "r"
            } else {
                "-"
            },
            if self.locked { "l" } else { "-" },
        ))
    }
}

impl From<PhysMemArea> for PmpStatus {
    fn from(value: PhysMemArea) -> Self {
        let ty = if value.get_region().len() == 0 {
            Mode::OFF
        } else if (value.get_region().len() & (value.get_region().len() - 1)) == 0 {
            Mode::NAPOT
        } else {
            Mode::TOR
        };

        Self {
            region: value.region,
            mode: ty,
            permission: value.prop.get_owner_perm(),
            locked: false,
        }
    }
}

pub fn calc_napot_area(target: usize, bottom: usize, top: usize) -> Range<usize> {
    let bottom_length = bit_length(target ^ bottom);
    let top_length = bit_length(target ^ top);

    let mut length = if bottom_length <= top_length {
        bottom_length
    } else {
        top_length
    };

    loop {
        let new_length = length + 1;
        let mask = !((1 << new_length) - 1);
        let left = target & mask;
        let right = match (target & mask).checked_add(1 << new_length) {
            Some(right) => right,
            None => {
                length = 13;
                break;
            }
        };
        if left >= bottom && right <= top && left <= target {
            length = new_length;
        } else {
            break;
        }
    }

    let mask = !((1 << length) - 1);
    (target & mask)..((target & mask) + (1 << length))
}

fn bit_length(num: usize) -> usize {
    if num == 0 { 0 } else { num.ilog2() as usize }
}

#[cfg(test)]
mod test {
    use super::parser_napot;

    #[test]
    pub fn test_parser_napot() {
        let addr = 0b11011110111;
        let ra = parser_napot(addr);
        assert_eq!(ra, 0x1bc0..(0x1bc0 + 64));
    }
}
