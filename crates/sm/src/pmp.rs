use core::{fmt, ops::Range};
use heapless::Vec;
use console::log;
use macros::usize_env_or;
use riscv::{
    asm::sfence_vma_all,
    register::{Permission, Pmp, pmpcfg0, pmpcfg2},
};

use pma::PhysMemArea;

pub const PMP_COUNT: usize = usize_env_or!("PMP_COUNT", 16);

pub type PmpTy = riscv::register::Range;

pub fn reset_pmp_registers() {
    let all_pmp = PmpStatusGroup::default();
    unsafe { flush_pmp(all_pmp.into_iter()) }
}

// pub fn update_pmp_by_pmas(pmas: Vec<PhysMemArea, PMP_COUNT>) {
//     let new_hps = gen_hart_pmp_status(pmas);

//     unsafe { flush_pmp(new_hps.into_iter()) }
// }

pub fn hps_from_regs() -> Vec<PmpStatus, PMP_COUNT> {
    let mut hps = Vec::new();
    let mut prev_pmp: Option<PmpStatus> = None;
    for i in 0..PMP_COUNT {
        let mut s = PmpStatus::from_register(i);

        if i > 0 && s.is_tor() {
            if let Some(mut prev) = prev_pmp {
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

// fn gen_hart_pmp_status(pmas: Vec<PhysMemArea, PMP_COUNT>) -> Vec<PmpStatus, PMP_COUNT> {
//     let working_hps = hps_from_regs();
//     let mut new_hps: Vec<_, PMP_COUNT> = Vec::new();

//     for pma in pmas.into_iter() {
//         // let ty = if pma.
//         let status = PmpStatus::from_pma(pma);
//         new_hps.push(status).unwrap();
//     }

//     let mut cnt = 0;
//     let size = new_hps.len();
//     let mut remain_size = PMP_COUNT - size;
//     let mut prev_pmp: Option<PmpStatus> = None;

//     for s0 in working_hps.into_iter() {
//         let current_pmp = Some(s0.clone());
//         if cnt == 0 {
//             cnt += 1;
//             prev_pmp = current_pmp.clone();
//             if s0.is_tor() {
//                 continue;
//             }
//         }

//         if !new_hps
//             .iter()
//             .any(|s1| s1.region.start <= s0.region.start && s1.region.end >= s0.region.end)
//         {
//             if s0.is_tor() && remain_size >= 2 {
//                 if let Some(prev) = prev_pmp.clone() {
//                     new_hps.push(prev).unwrap();
//                     remain_size -= 1;
//                     new_hps.push(s0.clone()).unwrap();
//                     remain_size -= 1;
//                 }
//             } else if s0.is_napot() && remain_size != 0 {
//                 new_hps.push(s0.clone()).unwrap();
//                 remain_size -= 1;
//             }
//         }

//         prev_pmp = current_pmp;
//     }

//     let mut indices_to_remove: Vec<usize, PMP_COUNT> = Vec::new();
//     for (i, s1) in new_hps.iter().enumerate() {
//         if s1.is_off() {
//             continue;
//         }
//         if new_hps.iter().enumerate().any(|(j, s2)| {
//             i != j
//                 && !s2.is_off()
//                 && s1.region.start >= s2.region.start
//                 && s1.region.end <= s2.region.end
//         }) {
//             indices_to_remove.push(i).unwrap();
//             if s1.is_tor() && i > 0 {
//                 indices_to_remove.push(i - 1).unwrap();
//             }
//         }
//     }

//     let mut new_hps_f: Vec<PmpStatus, PMP_COUNT> = Vec::new();
//     for (i, s) in new_hps.iter().enumerate() {
//         if !indices_to_remove.contains(&i) {
//             new_hps_f.push(s.clone()).unwrap();
//         }
//     }

//     return new_hps_f;
// }

pub unsafe fn flush_pmp(pmps: impl Iterator<Item = PmpStatus>) {
    let mut idx = 0;

    for i in 0..PMP_COUNT {
        unsafe { PmpStatus::new().off().apply(i) };
    }

    // let mut prev_pmp: Option<PmpStatus> = None;
    for mut pmp in pmps {
        match pmp.get_ty() {
            PmpTy::NAPOT => {
                pmp.napot().apply(idx);
            }
            PmpTy::OFF => {
                pmp.off().apply(idx);
            }
            PmpTy::TOR => {
                // if idx > 0 {
                //     if let Some(mut prev) = prev_pmp {
                //         prev.off().apply(idx - 1);
                //     }
                // }
                pmp.tor().apply(idx);
            }
            _ => panic!("Unsupported PMP type"),
        }
        // prev_pmp = Some(pmp);
        idx += 1;
    }

    sfence_vma_all();
    // PmpStatusGroup::from_registers().print();
}

#[derive(Debug)]
pub struct PmpStatusGroup {
    status: Vec<PmpStatus, PMP_COUNT>,
}

impl PmpStatusGroup {
    pub fn from_registers() -> Self {
        let mut status = Vec::new();
        for i in 0..PMP_COUNT {
            let s = PmpStatus::from_register(i);
            status.push(s).unwrap();
        }

        Self { status }
    }

    pub fn into_iter(self) -> impl Iterator<Item = PmpStatus> {
        self.status.into_iter()
    }

    pub fn print(&self) {
        for (idx, i) in self.status.iter().enumerate() {
            log::debug!("{idx:>2}. {i}");
        }
    }
}

impl Default for PmpStatusGroup {
    fn default() -> Self {
        let mut status = Vec::new();
        let mut pmp = PmpStatus::new();
        pmp.region(0x8000_0000..0x8000_1000).napot();
        status.push(pmp).unwrap();

        Self { status }
    }
}

/// Read pmpcfg register
fn pmpcfg(idx: usize) -> Pmp {
    if idx < 8 {
        pmpcfg0::read().into_config(idx)
    } else if idx < 16 {
        pmpcfg2::read().into_config(idx - 8)
    } else {
        panic!()
    }
}

/// Read pmpaddr register
fn pmpaddr(idx: usize) -> usize {
    use riscv::register::*;
    match idx {
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
        _ => todo!(),
    }
}

fn pmp_region(idx: usize) -> Option<Range<usize>> {
    let mut region = 0..0;
    let cfg = pmpcfg(idx);
    let addr = pmpaddr(idx);
    match cfg.range {
        PmpTy::OFF => {
            region.start = (addr.clone()) << 2;
            region.end = (addr.clone()) << 2;
        }
        PmpTy::TOR => {
            region.end = addr << 2;
            if idx == 0 {
                log::debug!("TOR mode idx 0, forcing start to 0x00000000");
                region.start = 0;
            } else {
                region.start = pmpaddr(idx - 1) << 2;
            }
        }
        PmpTy::NA4 => {
            region = parser_na4(addr);
        }
        PmpTy::NAPOT => {
            region = parser_napot(addr);
        }
    }

    Some(region)
}

fn parser_napot(addr: usize) -> Range<usize> {
    let len = addr.trailing_ones();
    if len == 64 {
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
    pub ty: PmpTy,
    pub permission: Permission,
    pub locked: bool,
}

impl PmpStatus {
    pub const fn new() -> Self {
        Self {
            region: 0..0,
            ty: PmpTy::OFF,
            permission: Permission::NONE,
            locked: false,
        }
    }

    pub fn from_pma(pma: PhysMemArea, ty: PmpTy) -> Self {
        Self {
            region: pma.region,
            ty,
            permission: pma.prop.get_owner_perm(),
            locked: false,
        }
    }

    pub fn get_ty(&self) -> PmpTy {
        self.ty
    }

    pub fn from_register(idx: usize) -> Self {
        let cfg = pmpcfg(idx);
        Self {
            // idx,
            region: pmp_region(idx).unwrap_or(0..0),
            ty: cfg.range,
            permission: cfg.permission,
            locked: cfg.locked,
        }
    }

    pub fn get_region(&self) -> Range<usize> {
        self.region.clone()
    }

    pub fn is_tor(&self) -> bool {
        self.ty as usize == PmpTy::TOR as usize
    }

    pub fn is_off(&self) -> bool {
        self.ty as usize == PmpTy::OFF as usize
    }

    pub fn is_napot(&self) -> bool {
        self.ty as usize == PmpTy::NAPOT as usize
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
        self.ty = PmpTy::OFF;
        self
    }

    #[inline(always)]
    pub fn napot(&mut self) -> &mut Self {
        self.ty = PmpTy::NAPOT;
        self
    }

    #[inline(always)]
    pub fn tor(&mut self) -> &mut Self {
        self.ty = PmpTy::TOR;
        self
    }

    #[inline(always)]
    pub fn contains(&self, addr: usize) -> bool {
        self.region.contains(&addr)
    }

    #[inline(always)]
    pub fn idx(&mut self, idx: usize) -> &mut Self {
        self
    }

    pub unsafe fn apply(&self, idx: usize) {
        use riscv::register::*;

        let addr_bits: usize;

        match self.ty {
            PmpTy::NAPOT => {
                addr_bits =
                    (self.region.start >> 2) + ((self.region.end - self.region.start) >> 3) - 1;
            }
            PmpTy::OFF => {
                addr_bits = self.region.start >> 2;
            }
            PmpTy::TOR => {
                addr_bits = self.region.end >> 2;
            }
            _ => {
                panic!()
            }
        }

        match idx {
            0x0 => {
                pmpcfg0::set_pmp(0, self.ty, self.permission, self.locked);
                pmpaddr0::write(addr_bits);
            }
            0x1 => {
                pmpcfg0::set_pmp(1, self.ty, self.permission, self.locked);
                pmpaddr1::write(addr_bits);
            }
            0x2 => {
                pmpcfg0::set_pmp(2, self.ty, self.permission, self.locked);
                pmpaddr2::write(addr_bits);
            }
            0x3 => {
                pmpcfg0::set_pmp(3, self.ty, self.permission, self.locked);
                pmpaddr3::write(addr_bits);
            }
            0x4 => {
                pmpcfg0::set_pmp(4, self.ty, self.permission, self.locked);
                pmpaddr4::write(addr_bits);
            }
            0x5 => {
                pmpcfg0::set_pmp(5, self.ty, self.permission, self.locked);
                pmpaddr5::write(addr_bits);
            }
            0x6 => {
                pmpcfg0::set_pmp(6, self.ty, self.permission, self.locked);
                pmpaddr6::write(addr_bits);
            }
            0x7 => {
                pmpcfg0::set_pmp(7, self.ty, self.permission, self.locked);
                pmpaddr7::write(addr_bits);
            }
            0x8 => {
                pmpcfg2::set_pmp(0, self.ty, self.permission, self.locked);
                pmpaddr8::write(addr_bits);
            }
            0x9 => {
                pmpcfg2::set_pmp(1, self.ty, self.permission, self.locked);
                pmpaddr9::write(addr_bits);
            }
            0xa => {
                pmpcfg2::set_pmp(2, self.ty, self.permission, self.locked);
                pmpaddr10::write(addr_bits);
            }
            0xb => {
                pmpcfg2::set_pmp(3, self.ty, self.permission, self.locked);
                pmpaddr11::write(addr_bits);
            }
            0xc => {
                pmpcfg2::set_pmp(4, self.ty, self.permission, self.locked);
                pmpaddr12::write(addr_bits);
            }
            0xd => {
                pmpcfg2::set_pmp(5, self.ty, self.permission, self.locked);
                pmpaddr13::write(addr_bits);
            }
            0xe => {
                pmpcfg2::set_pmp(6, self.ty, self.permission, self.locked);
                pmpaddr14::write(addr_bits);
            }
            0xf => {
                pmpcfg2::set_pmp(7, self.ty, self.permission, self.locked);
                pmpaddr15::write(addr_bits);
            }
            _ => {
                log::info!("{idx}");
                todo!("current only support 0-15")
            }
        }
    }
}

impl fmt::Display for PmpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let start = self.region.start;
        let end = self.region.end;
        f.write_fmt(format_args!(
            "{}{:#010x}-{:#010x} ({}{}{}{})",
            if let PmpTy::OFF = self.ty { " " } else { "*" },
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
            PmpTy::OFF
        } else if (value.get_region().len() & (value.get_region().len() - 1)) == 0 {
            PmpTy::NAPOT
        } else {
            PmpTy::TOR
        };

        Self {
            region: value.region,
            ty,
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
