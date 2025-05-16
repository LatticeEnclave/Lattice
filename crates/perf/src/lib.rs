#![no_std]

use core::{fmt::Display, ops::Add};
use riscv::register::mcountinhibit;
use sbi::profiling::*;

pub type TimeRecord = Record<TimeProfiler>;
pub type CycleRecord = Record<CycleProfiler>;
pub type InstRecord = Record<InstProfiler>;

pub struct Record<M: Profiler> {
    monitor: M,
    total: M::Data,
}

impl<M: Profiler + Default> Default for Record<M> {
    fn default() -> Self {
        Self {
            monitor: M::default(),
            total: M::Data::default(),
        }
    }
}

impl<M: Profiler> Record<M> {
    pub fn new(monitor: M) -> Self {
        Self {
            monitor,
            total: M::Data::default(),
        }
    }

    #[inline]
    pub fn begin(&mut self) -> &mut Self {
        self.monitor.start();
        self
    }

    #[inline]
    pub fn end(&mut self) -> &mut Self {
        self.monitor.stop();
        self.total = self.total.add(self.monitor.delta());
        self
    }

    #[inline]
    pub fn clear(&mut self) -> &mut Self {
        self.total = M::Data::default();
        self
    }

    #[inline]
    pub fn get_data(&self) -> M::Data {
        self.total
    }
}

pub trait Profiler {
    type Data: Add<Output = Self::Data> + Copy + Default;

    fn start(&mut self);
    fn stop(&mut self);
    fn delta(&mut self) -> Self::Data;
}

pub struct InstProfiler {
    monitoring: bool,
    start: usize,
    end: usize,
}

impl Default for InstProfiler {
    fn default() -> Self {
        Self {
            monitoring: false,
            start: 0,
            end: 0,
        }
    }
}

impl Profiler for InstProfiler {
    type Data = usize;

    fn start(&mut self) {
        if self.monitoring {
            return;
        }
        self.monitoring = true;
        self.start = get_instret();
        unsafe { mcountinhibit::clear_ir() };
    }

    fn stop(&mut self) {
        if !self.monitoring {
            return;
        }
        self.monitoring = false;
        self.end = get_instret();
        unsafe { mcountinhibit::set_ir() };
    }

    fn delta(&mut self) -> Self::Data {
        let mut end = self.end;
        if self.monitoring {
            end = get_instret();
        }
        if end < self.start {
            end + (usize::MAX - self.start)
        } else {
            end - self.start
        }
    }
}

pub struct CycleProfiler {
    start: usize,
    end: usize,
}

impl Default for CycleProfiler {
    fn default() -> Self {
        Self { start: 0, end: 0 }
    }
}

impl Profiler for CycleProfiler {
    type Data = usize;

    fn start(&mut self) {
        self.start = get_cycle();
        unsafe { mcountinhibit::clear_cy() };
    }

    fn stop(&mut self) {
        unsafe { mcountinhibit::set_cy() };
        self.end = get_cycle();
    }

    fn delta(&mut self) -> Self::Data {
        let end = get_cycle();
        let res = if end < self.start {
            end + (usize::MAX - self.start)
        } else {
            end - self.start
        };
        self.start = end;
        res
    }
}

pub struct TimeProfiler {
    monitoring: bool,
    start: usize,
    end: usize,
}

impl Default for TimeProfiler {
    fn default() -> Self {
        Self {
            monitoring: false,
            start: 0,
            end: 0,
        }
    }
}

impl Profiler for TimeProfiler {
    type Data = usize;

    fn start(&mut self) {
        if self.monitoring {
            return;
        }
        self.monitoring = true;
        self.start = get_time();
    }

    fn stop(&mut self) {
        if !self.monitoring {
            return;
        }
        self.monitoring = false;
        self.end = get_time();
    }

    fn delta(&mut self) -> Self::Data {
        let mut end = self.end;
        if self.monitoring {
            end = get_time();
        }
        if end < self.start {
            end + (usize::MAX - self.start)
        } else {
            end - self.start
        }
    }
}

fn calc_cycle_per_fault(prev: f64, num: usize, delta: usize) -> f64 {
    (prev + ((delta as f64) / (num as f64 - 1.))) / ((num as f64) / (num as f64 - 1.))
}

#[derive(Default)]
pub struct PmpFaultRecord {
    cycle: CycleProfiler,
    pub cycle_per_fault: f64,
    pub cycle2_per_fault: f64,
    pub num: usize,
}

impl PmpFaultRecord {
    #[inline]
    pub fn empty() -> Self {
        Self {
            cycle: CycleProfiler::default(),
            cycle_per_fault: 0.0,
            cycle2_per_fault: 0.0,
            num: 0,
        }
    }

    #[inline]
    pub fn start_handle(&mut self) -> usize {
        // let delta = self.cycle.delta();
        // self.cycle.stop();
        self.num += 1;
        // if self.num == 1 {
        //     self.cycle_per_fault = delta as f64;
        // } else {
        //     self.cycle_per_fault = calc_cycle_per_fault(self.cycle_per_fault, self.num, delta);
        // }
        // delta
        0
    }

    #[inline]
    pub fn finish_handle(&mut self) -> usize {
        // let delta = self.cycle.delta();
        // self.cycle.start();
        // delta
        0
    }

    pub fn start(&mut self) {
        self.cycle.start();
    }
}
