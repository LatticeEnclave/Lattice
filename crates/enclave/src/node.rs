use crate::{EnclaveId, EnclaveIdx, EnclaveType, LinuxUserEnclave};
use core::ptr::NonNull;
use data_structure::linked_list::Node;
use spin::Mutex;

// pub enum EnclaveRef {
//     User(&'static mut LinuxUserEnclave),
//     Driver(&'static mut LinuxDriverEnclave),
// }

// impl EnclaveRef {
//     pub fn from_idx(idx: EnclaveIdx) -> Self {
//         let node = EncListNode::from_idx(idx);
//         node.as_enclave()
//     }
// }

pub type EncListNode = Node<EnclaveType>;
// pub struct EncListNode(Node<EnclaveType>);

// impl core::ops::Deref for EncListNode {
//     type Target = EnclaveType;

//     fn deref(&self) -> &Self::Target {
//         &self.0.value
//     }
// }

// impl core::ops::DerefMut for EncListNode {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0.value
//     }
// }

// impl From<Node<EnclaveType>> for EncListNode {
//     fn from(value: Node<EnclaveType>) -> Self {
//         Self(value)
//     }
// }

// impl EncListNode {
//     pub fn uninit_at(addr: usize) -> &'static mut Self {
//         let enclave = unsafe { &mut *(addr as *mut Self) };
//         enclave
//     }

//     pub fn get_type(&self) -> EnclaveType {
//         self.0.value
//     }

//     pub fn idx(&self) -> EnclaveIdx {
//         EnclaveIdx(self as *const EncListNode as usize)
//     }

//     pub fn from_idx(idx: EnclaveIdx) -> &'static mut Self {
//         unsafe {
//             let enclave = &mut *(idx.0 as *mut Self);
//             enclave
//         }
//     }

//     pub fn as_lue(&mut self) -> Option<&'static mut LinuxUserEnclave> {
//         // Safety: We have checked the type of the enclave
//         if self.is_user() {
//             Some(unsafe { LinuxUserEnclave::from_node(self) })
//         } else {
//             None
//         }
//     }

//     pub fn as_lde(&mut self) -> Option<&'static mut LinuxDriverEnclave> {
//         if self.is_driver() {
//             Some(unsafe { LinuxDriverEnclave::from_node(self) })
//         } else {
//             None
//         }
//     }

//     pub fn as_enclave(&mut self) -> EnclaveRef {
//         if self.is_user() {
//             EnclaveRef::User(unsafe { LinuxUserEnclave::from_node(self) })
//         } else {
//             EnclaveRef::Driver(unsafe { LinuxDriverEnclave::from_node(self) })
//         }
//     }

//     pub fn set_user(&mut self) {
//         self.to_user();
//     }

//     pub fn set_driver(&mut self) {
//         self.to_driver();
//     }

//     pub fn as_node_mut(&mut self) -> &mut Node<EnclaveType> {
//         &mut self.0
//     }

//     pub fn get_id(&mut self) -> EnclaveId {
//         match self.as_enclave() {
//             EnclaveRef::User(enclave) => enclave.id,
//             EnclaveRef::Driver(enclave) => enclave.id,
//         }
//     }

//     pub fn nonnull2ptr(ptr: NonNull<Node<EnclaveType>>) -> *const Self {
//         ptr.as_ptr() as *const _
//     }
// }
