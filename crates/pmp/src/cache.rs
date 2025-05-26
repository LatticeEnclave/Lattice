use console::log;
use pma::PhysMemAreaMgr;

use crate::{PmpRegGroup, PmpStatus};

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
