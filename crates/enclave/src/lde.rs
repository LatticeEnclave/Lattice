use data_structure::linked_list::LinkedList;

use crate::{Enclave, EnclaveData, EnclaveType};

pub type LinuxDriverEnclave = Enclave<LinuxDriver>;
pub struct LinuxDriverEnclaveList(LinkedList<EnclaveType>);

impl LinuxDriverEnclaveList {
    pub fn new() -> Self {
        Self(LinkedList::new())
    }
}

pub struct LinuxDriver {}

impl EnclaveData for LinuxDriver {
    const TYPE: EnclaveType = EnclaveType::Driver;
}
