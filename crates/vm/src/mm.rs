use bit_field::BitField;

use crate::{
    pm::{PhysAddr, PhysPageNum},
    vm::{VirtAddr, VirtPageNum},
};

/// 内存模型抽象，包含内存模型中的通用内容
pub trait MemModel: Copy + Default {
    const PTE_SIZE: usize;
    const PAGE_SIZE: usize;
    const LEVEL: usize;
    const ID: usize;
    const MAX: usize;

    fn from_vpns(vpns: [usize; 5]) -> VirtAddr;

    fn split_vpn(vpn: VirtPageNum) -> [usize; 5];

    fn get_vpn(vaddr: VirtAddr) -> VirtPageNum;

    fn get_offset(vaddr: VirtAddr, level: usize) -> usize;

    fn concat_paddr(ppn: PhysPageNum, offset: usize) -> PhysAddr;

    fn mode() -> riscv::register::satp::Mode;
}

#[derive(Clone, Copy, Default)]
pub struct SV39;

impl MemModel for SV39 {
    const PTE_SIZE: usize = 8;
    const PAGE_SIZE: usize = 0x1000;
    const LEVEL: usize = 3;
    const ID: usize = 8;
    const MAX: usize = 0x1 << 39;

    fn split_vpn(vpn: VirtPageNum) -> [usize; 5] {
        [
            vpn.0.get_bits(0..=8),
            vpn.0.get_bits(9..=17),
            vpn.0.get_bits(18..=26),
            0,
            0,
        ]
    }

    fn get_vpn(vaddr: VirtAddr) -> VirtPageNum {
        VirtPageNum(vaddr.0.get_bits(12..39))
    }

    fn get_offset(vaddr: VirtAddr, level: usize) -> usize {
        let hi = 12 + level * 9;
        vaddr.0.get_bits(0..hi)
    }

    fn concat_paddr(ppn: PhysPageNum, offset: usize) -> PhysAddr {
        let paddr = ppn.0 << 12 | offset;
        PhysAddr(paddr)
    }

    fn from_vpns(vpns: [usize; 5]) -> VirtAddr {
        let mut vpn = 0;

        vpn.set_bits(0..=8, vpns[0]);
        vpn.set_bits(9..=17, vpns[1]);
        vpn.set_bits(18..=26, vpns[2]);

        VirtAddr(vpn)
    }

    fn mode() -> riscv::register::satp::Mode {
        riscv::register::satp::Mode::Sv39
    }
}

#[derive(Clone, Copy, Default)]
pub struct SV48;

impl MemModel for SV48 {
    const PTE_SIZE: usize = 8;
    const PAGE_SIZE: usize = 0x1000;
    const LEVEL: usize = 4;
    const ID: usize = 9;
    const MAX: usize = 0x1 << 48;

    fn split_vpn(vpn: VirtPageNum) -> [usize; 5] {
        [
            vpn.0.get_bits(0..=8),
            vpn.0.get_bits(9..=17),
            vpn.0.get_bits(18..=26),
            vpn.0.get_bits(27..=35),
            0,
        ]
    }

    fn get_vpn(vaddr: VirtAddr) -> VirtPageNum {
        VirtPageNum(vaddr.0.get_bits(12..48))
    }

    fn get_offset(vaddr: VirtAddr, level: usize) -> usize {
        let hi = 12 + level * 9;
        vaddr.0.get_bits(0..hi)
    }

    fn concat_paddr(ppn: PhysPageNum, offset: usize) -> PhysAddr {
        let paddr = ppn.0 << 12 | offset;
        PhysAddr(paddr)
    }

    fn from_vpns(vpns: [usize; 5]) -> VirtAddr {
        let mut vpn = 0;

        vpn.set_bits(0..=8, vpns[0]);
        vpn.set_bits(9..=17, vpns[1]);
        vpn.set_bits(18..=26, vpns[2]);
        vpn.set_bits(27..=35, vpns[3]);

        VirtAddr(vpn)
    }

    fn mode() -> riscv::register::satp::Mode {
        riscv::register::satp::Mode::Sv48
    }
}
