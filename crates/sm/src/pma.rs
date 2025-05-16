use bit_field::BitField;
use core::{cmp::Ordering, fmt::Display, ops::Range, slice};
use enclave::EnclaveId;
use htee_console::{log, println};
use htee_device::device::MemRegion;
use nostd_rbtree::{NodePtr, RBTree, RBTreeAllocator, node_size};

use riscv::register::Permission;

use crate::{Error, consts::GUARD_PAGE_SIZE};

trait PmaExt {
    fn get_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>>;
    fn get_key_value_pma_ext(&self, addr: usize) -> Option<(&usize, &PmaInfo)>;
    fn get_prev_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>>;
}

impl PmaExt for RBTree<usize, PmaInfo> {
    fn get_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match addr.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => {
                        if addr < (*temp.0).key + (*temp.0).value.size {
                            return Some(temp.clone());
                        }
                        res = temp;
                        &mut (*temp.0).right
                    }
                    Ordering::Equal => return Some(temp.clone()),
                };
                if next.is_null() {
                    break;
                }
                temp = next;
            }
        }

        if res.is_null() {
            return None;
        } else {
            return Some(res.clone());
        }
    }

    fn get_key_value_pma_ext(&self, addr: usize) -> Option<(&usize, &PmaInfo)> {
        self.get_node_pma_ext(addr)
            .map(|node| unsafe { (&(*node.0).key, &(*node.0).value) })
    }

    fn get_prev_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>> {
        let a = 1..=2;
        todo!()
    }
}

#[derive(Clone)]
struct PmaInfo {
    size: usize,
    prop: PmaProp,
}

#[derive(Debug, Clone)]
pub struct PhysMemArea {
    pub region: Range<usize>,
    pub prop: PmaProp,
}

impl PhysMemArea {
    pub fn get_region(&self) -> Range<usize> {
        self.region.clone()
    }

    pub fn get_prop(&self) -> PmaProp {
        self.prop.clone()
    }

    pub fn check_owner(&self, eid: EnclaveId) -> bool {
        if self.prop.get_owner() == eid {
            return true;
        }
        if self.prop.get_owner() == EnclaveId::EVERYONE {
            return true;
        }

        false
    }
}

impl Display for PhysMemArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:#x}..{:#x}:{:#x}",
            self.region.start, self.region.end, self.prop.bits
        )
    }
}

/// Use a maple tree to manage the physical memory area
pub struct PhysMemAreaMgr {
    mtree: RBTree<usize, PmaInfo>,
    // for some platform, the minimal pmp granularity may be 0x1000
    align: u32,
}

impl PhysMemAreaMgr {
    pub fn uninit() -> Self {
        // let allocator = unsafe { ArrayAllocator::uninit() };
        let allocator = RBTreeAllocator::new(&mut [0_u8]);
        let mgr = Self {
            mtree: RBTree::new(allocator),
            align: 0x1000,
        };

        // mgr.insert_pma(PhysMemArea {
        //     region: device_mem.end..max_mem,
        //     prop: PmaProp::new(),
        // });

        mgr
    }

    /// Init pmamgr by given the memory range of device
    pub unsafe fn new(device_mem: &mut MemRegion) -> Self {
        const NODE_SIZE: usize = node_size::<usize, PmaProp>();

        let max_mem = device_mem.end() as usize;
        let node_num = max_mem / GUARD_PAGE_SIZE / 16;
        log::debug!("{node_num:#x}");

        let mp_size = node_num * NODE_SIZE;
        let mp_start = max_mem - mp_size;

        let allocator = RBTreeAllocator::new(unsafe {
            slice::from_raw_parts_mut(mp_start as *mut u8, mp_size)
        });

        log::debug!("pmp mgr arena: {:#x} - {:#x}", mp_start, max_mem);
        device_mem.size = mp_start - device_mem.start as usize;

        let mut mgr = Self {
            mtree: RBTree::new(allocator),
            align: 0x1000,
        };

        mgr.insert_pma(PhysMemArea {
            region: 0..(usize::MAX - 0x1),
            prop: PmaProp::default(),
        })
        .unwrap();

        mgr.insert_pma(PhysMemArea {
            region: mp_start..(mp_start + mp_size),
            prop: PmaProp::empty(),
        })
        .unwrap();

        mgr
    }

    pub fn iter_pma(&self) -> impl Iterator<Item = PhysMemArea> + '_ {
        self.mtree.iter().map(|(start, info)| PhysMemArea {
            region: *start..(*start + info.size),
            prop: info.prop,
        })
    }

    pub fn get_pma(&self, addr: impl Into<usize>) -> Option<PhysMemArea> {
        let addr: usize = addr.into();
        self.mtree.get_key_value_pma_ext(addr).map(|(start, info)| {
            assert!(addr < start + info.size);
            let region = (*start)..(*start + info.size);
            log::trace!(
                "addr: {:#x}, pma: {:#x}..{:#x}:{}:{:#b}",
                addr,
                start,
                start + info.size,
                info.prop.get_owner(),
                info.prop.get_owner_perm() as usize,
            );
            PhysMemArea {
                region,
                prop: info.prop,
            }
        })
    }

    pub fn insert_pma(&mut self, pma: PhysMemArea) -> Result<(), Error> {
        if self.mtree.is_empty() {
            self.mtree.insert(pma.region.start, PmaInfo {
                size: pma.region.end - pma.region.start,
                prop: pma.prop,
            });
            return Ok(());
        }
        let node = self.mtree.get_node_pma_ext(pma.region.start).unwrap();
        let (k, v) = node.get_key_value().unwrap();

        if pma.region.end > (*k + v.size) {
            return Err("The region is beyond the end".into());
        }

        if pma.region.start == *k && pma.region.end == (*k + v.size) {
            // the new region is the same as the old one
            // so we just need to update the property
            // we also need to check the previous and next region, and merge them if they have the same property
            rbtree_replace_pma(&mut self.mtree, pma, node);
        } else if pma.region.start == *k {
            // the new region is smaller than the old one
            // but the new region has the same start address
            // so we need to reduce the old region, and insert the new region
            rbtree_insert_pma_begin(&mut self.mtree, pma, node);
        } else if pma.region.end == (*k + v.size) {
            // the new region is smaller than the old one
            // but the new region has the same end address
            // so we need to reduce the old region and keep the property, and insert the new region with new property
            rbtree_insert_pma_end(&mut self.mtree, pma, node);
        } else {
            // the new region is smaller than the old one
            // and the new region is in the middle of the old region
            // so we need to split the old region into two parts
            rbtree_insert_pma_middle(&mut self.mtree, pma, node);
        }

        Ok(())
    }

    pub fn insert_page(
        &mut self,
        paddr: impl Into<usize>,
        owner: impl Into<usize>,
        perm: impl Into<Permission>,
    ) {
        let paddr = paddr.into();
        let pma = PhysMemArea {
            region: paddr..(paddr + 0x1000),
            prop: PmaProp::empty().owner(owner.into()).permission(perm.into()),
        };

        self.insert_pma(pma).unwrap();
    }
}

fn rbtree_replace_pma(
    tree: &mut RBTree<usize, PmaInfo>,
    pma: PhysMemArea,
    node: NodePtr<usize, PmaInfo>,
) {
    let (_, current_info) = node.get_key_value().unwrap();
    current_info.prop = pma.prop;
    let current_node = if let Some(prev_node) = tree.get_prev_node(&pma.region.start) {
        rbtree_merge_node(tree, prev_node, node).unwrap_or(node)
    } else {
        node
    };

    if let Some(next_node) = tree.get_next_node(&pma.region.start) {
        rbtree_merge_node(tree, current_node, next_node);
    }
}

fn rbtree_insert_pma_begin(
    tree: &mut RBTree<usize, PmaInfo>,
    pma: PhysMemArea,
    node: NodePtr<usize, PmaInfo>,
) {
    let (_, current_info) = node.get_key_value().unwrap();
    let old_prop = current_info.prop;
    let old_size = current_info.size;
    current_info.prop = pma.prop;
    current_info.size = pma.region.end - pma.region.start;
    // merge the previous node if it has the same property
    if let Some(prev_node) = tree.get_prev_node(&pma.region.start) {
        rbtree_merge_node(tree, prev_node, node);
    }

    // Insert new node after the current node
    tree.insert(pma.region.end, PmaInfo {
        size: old_size - (pma.region.end - pma.region.start),
        prop: old_prop,
    });
}

fn rbtree_insert_pma_end(
    tree: &mut RBTree<usize, PmaInfo>,
    pma: PhysMemArea,
    node: NodePtr<usize, PmaInfo>,
) {
    let (start, current_info) = node.get_key_value().unwrap();

    // We only need to update the size of the current node
    current_info.size = pma.region.start - *start;

    // Insert new node after the current node
    tree.insert(pma.region.start, PmaInfo {
        size: pma.region.end - pma.region.start,
        prop: pma.prop,
    });

    // Merge the next node if it has the same property
    // let new_node = tree.get_prev_or_equal_node(&pma.region.start).unwrap();
    let new_node = tree.get_node_pma_ext(pma.region.start).unwrap();

    if let Some(next_node) = tree.get_next_node(&pma.region.start) {
        rbtree_merge_node(tree, new_node, next_node);
    }
}

fn rbtree_insert_pma_middle(
    tree: &mut RBTree<usize, PmaInfo>,
    pma: PhysMemArea,
    node: NodePtr<usize, PmaInfo>,
) {
    let (start, current_info) = node.get_key_value().unwrap();
    let old_prop = current_info.prop;
    let old_size = current_info.size;
    let first_size = pma.region.start - start;
    let second_size = pma.region.end - pma.region.start;
    let third_size = *start + old_size - pma.region.end;

    // first part only need to update the size
    current_info.size = first_size;

    // insert the second part
    tree.insert(pma.region.start, PmaInfo {
        size: second_size,
        prop: pma.prop,
    });

    // insert the third part
    tree.insert(pma.region.end, PmaInfo {
        size: third_size,
        prop: old_prop,
    });
}

fn rbtree_merge_node(
    tree: &mut RBTree<usize, PmaInfo>,
    left: NodePtr<usize, PmaInfo>,
    right: NodePtr<usize, PmaInfo>,
) -> Option<NodePtr<usize, PmaInfo>> {
    let (left_k, left_v) = left.get_key_value().unwrap();
    let (right_k, right_v) = right.get_key_value().unwrap();
    // the two nodes must be adjacent
    if left_k + left_v.size != *right_k {
        return None;
    }
    // the two nodes must have the same property
    if left_v.prop != right_v.prop {
        return None;
    }
    // merge the right node into the left node
    left_v.size += right_v.size;
    // remove the right node
    tree.remove(right_k);
    return Some(left);
}

// fn rbtree_replace_pma(
//     tree: &mut RBTree<usize, PmaInfo>,
//     pma: PhysMemArea,
//     node: NodePtr<usize, PmaInfo>,
// ) {
//     let (_, current_info) = node.get_key_value().unwrap();
//     current_info.prop = pma.prop;
//     let current_node = if let Some(prev_node) = tree.get_prev_node(&pma.region.start) {
//         rbtree_merge_node(tree, prev_node, node).unwrap_or(node)
//     } else {
//         node
//     };

//     if let Some(next_node) = tree.get_next_node(&pma.region.start) {
//         rbtree_merge_node(tree, current_node, next_node);
//     }
// }

// fn rbtree_insert_pma_begin(
//     tree: &mut RBTree<usize, PmaInfo>,
//     pma: PhysMemArea,
//     node: NodePtr<usize, PmaInfo>,
// ) {
//     let (_, current_info) = node.get_key_value().unwrap();
//     let old_prop = current_info.prop;
//     let old_size = current_info.size;
//     current_info.prop = pma.prop;
//     current_info.size = pma.region.end - pma.region.start;
//     if let Some(prev_node) = tree.get_prev_node(&pma.region.start) {
//         rbtree_merge_node(tree, prev_node, node);
//     }

//     // Insert new node after the current node
//     tree.insert(
//         pma.region.end,
//         PmaInfo {
//             size: old_size - (pma.region.end - pma.region.start),
//             prop: old_prop,
//         },
//     );
// }

// fn rbtree_insert_pma_end(
//     tree: &mut RBTree<usize, PmaInfo>,
//     pma: PhysMemArea,
//     node: NodePtr<usize, PmaInfo>,
// ) {
//     let (start, current_info) = node.get_key_value().unwrap();

//     // We only need to update the size of the current node
//     current_info.size = pma.region.start - *start;

//     // Insert new node after the current node
//     tree.insert(
//         pma.region.start,
//         PmaInfo {
//             size: pma.region.end - pma.region.start,
//             prop: pma.prop,
//         },
//     );

//     // Merge the next node if it has the same property
//     let new_node = tree.get_prev_or_equal_node(&pma.region.start).unwrap();

//     if let Some(next_node) = tree.get_next_node(&pma.region.start) {
//         rbtree_merge_node(tree, new_node, next_node);
//     }
// }

// fn rbtree_insert_pma_middle(
//     tree: &mut RBTree<usize, PmaInfo>,
//     pma: PhysMemArea,
//     node: NodePtr<usize, PmaInfo>,
// ) {
//     let (start, current_info) = node.get_key_value().unwrap();
//     let old_prop = current_info.prop;
//     let old_size = current_info.size;
//     let first_size = pma.region.start - start;
//     let second_size = pma.region.end - pma.region.start;
//     let third_size = *start + old_size - pma.region.end;

//     // first part only need to update the size
//     current_info.size = first_size;

//     // insert the second part
//     tree.insert(
//         pma.region.start,
//         PmaInfo {
//             size: second_size,
//             prop: pma.prop,
//         },
//     );

//     // insert the third part
//     tree.insert(
//         pma.region.end,
//         PmaInfo {
//             size: third_size,
//             prop: old_prop,
//         },
//     );
// }

// fn rbtree_merge_node(
//     tree: &mut RBTree<usize, PmaInfo>,
//     left: NodePtr<usize, PmaInfo>,
//     right: NodePtr<usize, PmaInfo>,
// ) -> Option<NodePtr<usize, PmaInfo>> {
//     let (left_k, left_v) = left.get_key_value().unwrap();
//     let (right_k, right_v) = right.get_key_value().unwrap();
//     if left_k + left_v.size != *right_k {
//         return None;
//     }
//     if left_v.prop != right_v.prop {
//         return None;
//     }
//     // merge the right node into the left node
//     left_v.size += right_v.size;
//     // remove the right node
//     tree.remove(right_k);
//     return Some(left);
// }

/// 描述了一块PMA区域的所有者以及所有者拥有的权限
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PmaProp {
    bits: usize,
}

impl PmaProp {
    pub fn empty() -> Self {
        Self { bits: 0 }
    }

    pub fn everyone() -> Self {
        Self::empty().owner(EnclaveId::EVERYONE)
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn owner(mut self, owner: impl Into<usize>) -> Self {
        self.bits.set_bits(3..64, owner.into());
        self
    }

    pub fn permission(mut self, permission: impl Into<Permission>) -> Self {
        let perm: Permission = permission.into();
        let bits = match perm {
            Permission::NONE => 0b000,
            Permission::R => 0b001,
            Permission::W => 0b010,
            Permission::RW => 0b011,
            Permission::X => 0b100,
            Permission::RX => 0b101,
            Permission::WX => 0b110,
            Permission::RWX => 0b111,
        };
        self.bits.set_bits(0..=2, bits);
        self
    }

    pub fn get_owner(&self) -> EnclaveId {
        self.bits.get_bits(3..64).into()
    }

    pub fn get_owner_perm(&self) -> Permission {
        match self.bits.get_bits(0..=2) {
            0 => Permission::NONE,
            1 => Permission::R,
            2 => Permission::W,
            3 => Permission::RW,
            4 => Permission::X,
            5 => Permission::RX,
            6 => Permission::WX,
            7 => Permission::RWX,
            _ => unreachable!(),
        }
    }
}

impl Default for PmaProp {
    fn default() -> Self {
        Self::empty()
            .permission(Permission::RWX)
            .owner(EnclaveId::HOST)
    }
}
