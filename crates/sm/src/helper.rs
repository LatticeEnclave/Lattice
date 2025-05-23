use heapless::Vec;
use htee_console::log;
use vm::{
    BarePtReader, PhysAddr, PhysPageNum, Translate, VAddrTranslator, mm::MemModel, vm::VirtPageNum,
};

use crate::{Error, PMP_COUNT, PmpStatus};
use pma::{PhysMemArea, PhysMemAreaMgr};
use pmp::{MAX_PMP_COUNT, PmpHelper, calc_napot_area};

#[inline]
pub fn pmas_req_vaddr<M: MemModel>(
    mgr: &PhysMemAreaMgr,
    mepc: usize,
    mtval: usize,
    root_ppn: usize,
    mm: M,
    buf: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
) -> Result<(), Error> {
    let root_ppn = PhysPageNum(root_ppn);
    let root_paddr = PhysAddr::from_ppn(root_ppn).0;
    log::trace!("pt_root: {:#x}", root_paddr);
    let root_pma = mgr.get_pma(root_paddr).ok_or_else(|| {
        log::error!("pma for {:#x} not found", root_paddr);
        Error::InvalidAddress(root_paddr)
    })?;

    buf.push(PmpHelper {
        pma: root_pma,
        addr: root_paddr,
        is_tor: false,
    })
    .unwrap();

    let p_mepc = vm::VirtAddr(mepc)
        .translate(root_ppn, M::mode(), &BarePtReader)
        .ok_or_else(|| {
            // log::error!(
            //     "mepc: {:#x} can't be translated in pt_root: {:#x}",
            //     mepc,
            //     root_paddr
            // );
            Error::InvalidAddress(mepc)
        })?;
    log::trace!("walk mepc: {:#x} => {:#x}", mepc, p_mepc.0);
    for pte in
        VAddrTranslator::new(VirtPageNum::from_vaddr(mepc), root_ppn, &BarePtReader, mm).iter_pte()
    {
        let addr = if pte.is_leaf() {
            p_mepc.0
        } else {
            PhysAddr::from_ppn(pte.get_ppn()).0
        };
        if let Some(p) = buf.iter_mut().find(|p| p.pma.get_region().contains(&addr)) {
            p.is_tor = true;
        } else {
            let pma = mgr.get_pma(addr).ok_or_else(|| {
                log::error!("pma for {:#x} not found", addr);
                Error::InvalidAddress(addr)
            })?;
            buf.push(PmpHelper {
                pma,
                addr,
                is_tor: false,
            })
            .unwrap();
        }
    }

    if mtval != mepc {
        let p_mtval = vm::VirtAddr(mtval)
            .translate(root_ppn, M::mode(), &BarePtReader)
            .ok_or_else(|| {
                // log::error!(
                //     "mtval: {:#x} can't be translated in pt_root: {:#x}",
                //     mtval,
                //     root_paddr
                // );
                Error::InvalidAddress(mtval)
            })?;
        log::trace!("walk mtval: {:#x} => {:#x}", mtval, p_mtval.0);
        for pte in VAddrTranslator::new(VirtPageNum::from_vaddr(mtval), root_ppn, &BarePtReader, mm)
            .iter_pte()
        {
            let addr = if pte.is_leaf() {
                p_mtval.0
            } else {
                PhysAddr::from_ppn(pte.get_ppn()).0
            };
            if let Some(p) = buf.iter_mut().find(|p| p.pma.get_region().contains(&addr)) {
                p.is_tor = true;
            } else {
                let pma = mgr.get_pma(addr).ok_or_else(|| {
                    log::error!("pma for {:#x} not found", addr);
                    Error::InvalidAddress(addr)
                })?;
                buf.push(PmpHelper {
                    pma,
                    addr,
                    is_tor: false,
                })
                .unwrap();
            }
        }
    }

    Ok(())
}

#[inline(always)]
pub fn update_pmp_by_pmas(
    helpers: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
    current_pmas: impl Iterator<Item = PhysMemArea>,
) {
    use pmp::flush_pmp;

    let new_hps = gen_hart_pmp_status(helpers, current_pmas);
    unsafe { flush_pmp(new_hps) }

    log::trace!("pmp register status:");
    let current_hps = pmp::hps_from_regs();
    current_hps.iter().for_each(|p| log::trace!("{p}"));
}

#[inline]
fn gen_hart_pmp_status(
    buf: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
    current_pmas: impl Iterator<Item = PhysMemArea>,
) -> impl Iterator<Item = PmpStatus> {
    use pmp::{Mode, PmpStatus};

    for pma in current_pmas {
        if let Some(p) = buf
            .iter_mut()
            .find(|p| p.pma.get_region().contains(&pma.get_region().start))
        {
            p.is_tor = true;
        } else {
            buf.push(PmpHelper {
                addr: pma.get_region().start,
                pma,
                is_tor: true,
            })
            .unwrap();
        }

        if buf.is_full() {
            break;
        }
    }

    let mut remain_size = PMP_COUNT;

    buf.into_iter().filter_map(move |p| {
        let mut p = p.clone();
        if remain_size == 0 {
            return None;
        }
        let napot_region = calc_napot_area(p.addr, p.pma.region.start, p.pma.region.end);
        if napot_region == p.pma.region {
            p.is_tor = false;
        }
        if remain_size == 1 {
            p.is_tor = false;
        }
        if p.is_tor {
            remain_size -= 2;
            Some(PmpStatus::from_pma(p.pma, Mode::TOR))
        } else {
            remain_size -= 1;
            p.pma.region = napot_region;
            Some(PmpStatus::from_pma(p.pma, Mode::NAPOT))
        }
    })
}

#[inline]
pub fn pmas_on_paddr(
    mgr: &PhysMemAreaMgr,
    mepc: usize,
    mtval: usize,
    buf: &mut Vec<PmpHelper, MAX_PMP_COUNT>,
) -> Result<(), Error> {
    log::trace!("mepc: {:#x}", mepc);
    let pma_mepc = mgr.get_pma(mepc).ok_or_else(|| {
        log::error!("pma for {:#x} not found", mepc);
        Error::InvalidAddress(mepc)
    })?;
    buf.push(PmpHelper {
        addr: mepc,
        pma: pma_mepc,
        is_tor: false,
    })
    .unwrap();

    if mepc == mtval {
        return Ok(());
    }

    log::trace!("mtval: {:#x}", mtval);
    let pma_mtval = mgr.get_pma(mtval).ok_or_else(|| {
        log::error!("pma for {:#x} not found", mtval);
        Error::InvalidAddress(mtval)
    })?;
    buf.push(PmpHelper {
        addr: mtval,
        pma: pma_mtval,
        is_tor: false,
    })
    .unwrap();

    // if pma_mepc.region == pma_mtval.region {
    //     // let napot_region = calc_napot_area(mepc, pma_mepc.region.start, pma_mepc.region.end);
    //     if napot_region == pma_mepc.region {
    //         // can be napot mode
    //         pmas.push(pma_mepc).unwrap();
    //     } else {
    //         // tor mode
    //         let mut pma_mepc_off = pma_mepc.clone();
    //         pma_mepc_off.region = pma_mepc.region.start..pma_mepc.region.start;
    //         pmas.push(pma_mepc_off).unwrap();
    //         pmas.push(pma_mepc).unwrap();
    //     }
    // } else {
    //     // napot
    //     pma_mepc.region = calc_napot_area(mepc, pma_mepc.region.start, pma_mepc.region.end);
    //     pma_mtval.region = calc_napot_area(mtval, pma_mtval.region.start, pma_mtval.region.end);
    //     pmas.push(pma_mepc).unwrap();
    //     pmas.push(pma_mtval).unwrap();
    // }

    Ok(())
}
