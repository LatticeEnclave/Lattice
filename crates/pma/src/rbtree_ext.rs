use core::cmp::Ordering;

use nostd_rbtree::{NodePtr, RBTree};

use crate::PmaInfo;

pub trait PmaExt {
    fn get_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>>;
    fn get_key_value_pma_ext(&self, addr: usize) -> Option<(&usize, &PmaInfo)>;
    // fn get_prev_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>>;
}

impl PmaExt for RBTree<usize, PmaInfo> {
    #[inline]
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

    #[inline(always)]
    fn get_key_value_pma_ext(&self, addr: usize) -> Option<(&usize, &PmaInfo)> {
        self.get_node_pma_ext(addr)
            .map(|node| unsafe { (&(*node.0).key, &(*node.0).value) })
    }

    // fn get_prev_node_pma_ext(&self, addr: usize) -> Option<NodePtr<usize, PmaInfo>> {
    //     let a = 1..=2;
    //     todo!()
    // }
}
