#![no_std]

mod prop;
mod rbtree_ext;

use core::{fmt::Display, ops::Range};

use console::log;
use nostd_rbtree::{NodePtr, RBTree, RBTreeAllocator, node_size};
use rbtree_ext::PmaExt;
use vm::{BarePtReader, Translate, VirtMemArea};

pub use prop::{Owner, PmaProp};

#[derive(Debug)]
pub enum Error {
    SizeOverflow,
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
    #[inline]
    pub fn get_region(&self) -> Range<usize> {
        self.region.clone()
    }

    #[inline]
    pub fn get_prop(&self) -> PmaProp {
        self.prop.clone()
    }

    #[inline]
    pub fn check_owner(&self, owner: impl FnOnce(Owner) -> bool) -> bool {
        owner(self.prop.get_owner())
    }
}

impl Display for PhysMemArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:#x}..{:#x}:{:#x}",
            self.region.start,
            self.region.end,
            self.prop.bits()
        )
    }
}

pub struct PhysMemAreaMgr {
    mtree: RBTree<usize, PmaInfo>,
}

impl PhysMemAreaMgr {
    pub const NODE_SIZE: usize = node_size::<usize, PmaProp>();

    #[inline(always)]
    pub fn uninit() -> Self {
        // let allocator = unsafe { ArrayAllocator::uninit() };
        let allocator = RBTreeAllocator::new(&mut [0_u8]);
        let mgr = Self {
            mtree: RBTree::new(allocator),
        };

        mgr
    }

    #[inline(always)]
    pub fn new(mem_pool: &mut [u8]) -> Self {
        let allocator = RBTreeAllocator::new(mem_pool);
        let mgr = Self {
            mtree: RBTree::new(allocator),
        };
        mgr
    }

    #[inline]
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
            return Err(Error::SizeOverflow);
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

    pub fn insert_page(&mut self, paddr: impl Into<usize>, prop: PmaProp) {
        let paddr = paddr.into();
        let pma = PhysMemArea {
            region: paddr..(paddr + 0x1000),
            prop,
        };

        self.insert_pma(pma).unwrap();
    }

    pub fn update_pma_by_vma(&mut self, vma: VirtMemArea, prop: PmaProp) {
        // let page_num = vma.size.div_ceil(0x1000);
        log::debug!(
            "enclave memory range: {:#x}-{:#x}",
            vma.start,
            vma.start + vma.size
        );

        // log::debug!("page table: {:#x}", pt_ppn.0);
        for vpn in vma.iter_vpn() {
            let paddr = vpn
                .translate(vma.satp.ppn(), vma.satp.mode(), &BarePtReader)
                .unwrap();
            self.insert_page(paddr, prop)
        }
    }
}

#[inline]
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

#[inline]
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

#[inline]
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

#[inline]
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

#[inline]
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
