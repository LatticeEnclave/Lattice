use data_structure::linked_list::LinkedList;
use htee_console::log;

use context::HartContext;

use crate::{Enclave, EnclaveData, EnclaveType};

use super::EnclaveId;

pub type LinuxUserEnclave = Enclave<LinuxUser>;

pub struct LinuxUserEnclaveList(LinkedList<EnclaveType>);

impl LinuxUserEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }

    pub fn get(&self, eid: EnclaveId) -> Option<&'static mut LinuxUserEnclave> {
        self.0.iter().find_map(|ptr| {
            // Safety: the node is pushed by &mut LinuxUserEnclave, thus it is valid
            let lue = unsafe { LinuxUserEnclave::from_ptr(ptr) };
            if lue.id == eid { Some(lue) } else { None }
        })
    }

    pub fn push(&mut self, lue: &'static mut LinuxUserEnclave) {
        debug_assert_eq!(lue.get_type(), EnclaveType::User);
        self.0.push_node(&mut lue.list.lock());
    }

    pub fn remove(&mut self, eid: EnclaveId) -> Option<&'static mut LinuxUserEnclave> {
        let enc = self
            .get(eid)
            .and_then(|enc| self.0.rm_node(&mut enc.list.lock()))
            .map(|node| unsafe { Enclave::from_ptr(node) })?;

        log::debug!("remaining enclaves:");
        for e in self.0.iter() {
            let eid = unsafe { Enclave::<()>::from_ptr(e).id };
            log::debug!("{eid}");
        }

        Some(enc)
    }
}

pub struct LinuxUser {
    pub enc_ctx: HartContext,
    pub pmp_cache: pmp::Cache,

    pub pause_num: usize,
    pub switch_cycle: perf::CycleRecord,
}

impl EnclaveData for LinuxUser {
    const TYPE: EnclaveType = EnclaveType::User;
}
