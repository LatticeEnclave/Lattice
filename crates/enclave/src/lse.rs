use data_structure::linked_list::LinkedList;
use vm::VirtMemArea;

use crate::{Enclave, EnclaveData, EnclaveType};

pub type LinuxServiceEnclave = Enclave<LinuxService>;

pub struct LinuxServiceEnclaveList(LinkedList<EnclaveType>);

impl LinuxServiceEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }

    pub fn first(&self) -> Option<&'static mut LinuxServiceEnclave> {
        self.0
            .iter()
            .next()
            .map(|ptr| unsafe { LinuxServiceEnclave::from_ptr(ptr) })
    }

    pub fn push(&mut self, lse: &'static mut LinuxServiceEnclave) {
        debug_assert_eq!(lse.get_type(), EnclaveType::Service);
        self.0.push_node(&mut lse.list.lock());
    }
}

pub struct LinuxService {
    pub rt: VirtMemArea,
    pub trampoline: VirtMemArea,
}

impl EnclaveData for LinuxService {
    const TYPE: EnclaveType = EnclaveType::Service;
}

impl LinuxService {}
