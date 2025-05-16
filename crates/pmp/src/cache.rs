use extension::Extension;
use heapless::Vec;
use htee_console::log;
use pma::PhysMemAreaMgr;

use crate::{PMP_COUNT, PmpRegGroup, PmpStatus};

pub struct Cache {
    entries: PmpRegGroup,
}

impl Cache {
    #[inline]
    pub fn dump(&mut self) {
        self.entries = PmpRegGroup::from_registers();
    }

    #[inline]
    pub fn restore(&self) {
        if !self.entries.0.is_empty() {
            unsafe { self.entries.flush() };
        }
    }
}

pub struct NwCache {
    cache: Cache,
}

impl NwCache {
    #[inline]
    pub fn clear(&mut self) {
        self.cache.entries.0.clear();
    }

    #[inline]
    pub fn push(&mut self, pmp: PmpStatus) {
        self.cache.entries.0.push(pmp).unwrap();
    }

    #[inline]
    pub fn restore(&self) {
        self.cache.restore();
    }
}

pub trait NwCacheExt: Extension<PhysMemAreaMgr> + Extension<NwCache> {
    #[inline]
    fn update_nw_pmp_cache(&self) {
        self.update(|nw_cache: &mut NwCache| {
            nw_cache.clear();
            for pmp in crate::iter_hps() {
                nw_cache.push(pmp);
            }
        })
    }

    #[inline]
    fn apply_nw_pmp_cache(&self) {
        log::debug!("apply nw pmp cache");
        self.view(|nw_cache: &NwCache| nw_cache.restore())
    }
}

impl<T> NwCacheExt for T where T: Extension<PhysMemAreaMgr> + Extension<NwCache> {}
