use extension::Extension;
use console::log;
use sbi::TrapRegs;

use crate::{EnclaveId, Error, LinuxUserEnclave, lue::LinuxUserEnclaveList};

pub trait EnclaveCtl {
    fn pause(&self, eid: EnclaveId, regs: &mut TrapRegs) -> Result<(), Error>;
    // fn exit(&self, eid: EnclaveId) -> Result<(), Error>;
}
