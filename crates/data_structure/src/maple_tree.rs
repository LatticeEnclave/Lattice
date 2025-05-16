use core::{marker::PhantomData, ops::Range};

use crate::array_alloc::ArrayAllocator;

pub struct MapleTree<T> {
    root: NodePtr<T>,
    allocator: ArrayAllocator<MapleNode<T>>,
    region: Range<usize>,
}

impl<T: Default + 'static> MapleTree<T> {
    pub const fn uninit() -> Self {
        Self {
            root: NodePtr::empty(),
            allocator: unsafe { ArrayAllocator::uninit() },
            region: 0..0,
        }
    }

    pub fn new(region: Range<usize>, allocator: ArrayAllocator<MapleNode<T>>) -> Self {
        Self {
            root: NodePtr::empty(),
            allocator,
            region,
        }
    }

    fn get_root(&self) -> &'static MapleNode<T> {
        self.root.as_ref()
    }

    fn get_value(&self, key: usize) -> Option<&T> {
        if self.root.is_empty() {
            return None;
        }
        let mut node = self.get_root();

        loop {
            let pos = node.search_key(key);

            if node.is_leaf() {
                return Some(node.entries[pos].get_leaf());
            }
            node = node.entries[pos].get_child_ptr().as_ref();
        }
    }

    /// Get the range and that the given key inside
    pub fn get_range(&self, key: usize) -> Option<(Range<usize>, &T)> {
        if self.root.is_empty() {
            return None;
        }

        let mut node = self.get_root();

        let mut start = self.region.start;
        let mut end = self.region.end;
        loop {
            let pos = node.search_end(key);

            if node.is_leaf() {
                if pos == 0 {
                    end = node.keys[pos];
                } else if pos == node.get_size() {
                    start = node.keys[pos - 1];
                } else {
                    start = node.keys[pos - 1];
                    end = node.keys[pos];
                }

                return Some((start..end, &node.entries[pos].get_leaf()));
            }
            if pos < node.get_size() {
                end = node.keys[pos];
            }
            if pos > 0 {
                start = node.keys[pos - 1];
            }

            node = node.entries[pos].get_child_ptr().as_ref();
        }
    }

    /// 二分法搜索目标索引
    fn search_path(&self, key: usize) -> MTreePath<T> {
        let mut node = self.get_root();

        let mut path = MTreePath::new();

        loop {
            let pos = node.search_key(key);

            // save current node as parent node
            path.push(node.as_ptr(), pos);

            if node.is_leaf() {
                return path;
            }

            // search child node
            node = node.entries[pos].get_child_ptr().as_ref();
        }
    }
}

impl<T: Default + Copy + 'static> MapleTree<T> {
    pub fn insert(&mut self, key: usize, value: T) {
        if key == self.region.start {
            panic!()
        }

        let entry = MapleEntry::new_leaf(value);
        // empty tree
        if self.root.is_empty() {
            let elem = self.allocator.alloc_elem().init();
            elem.set(0, key, entry);
            self.root = elem.as_ptr();
            return;
        }

        let path = self.search_path(key);

        mtree_insert_at_impl(&mut self.root, key, entry, path, &mut self.allocator);
    }

    pub fn remove(&mut self, key: usize) {
        if self.root.is_empty() {
            return;
        }

        let mut path = self.search_path(key);

        let (ptr, idx) = path.get_leaf();

        let node = ptr.as_mut();

        if node.keys[idx] != key {
            return;
        }

        if node.is_leaf() {
            node.remove_at(idx);
        } else {
            node.replace_predecessor_at(idx, &mut path);
        }

        // self.solve_underflow(path);
        mtree_solve_underflow(&mut self.root, path, &mut self.allocator);
    }
}

impl<T: Copy + Clone + Default + PartialEq + 'static> MapleTree<T> {
    /// Insert a range into the mtree
    pub fn insert_range(&mut self, range: Range<usize>, value: T) {
        if range.end == self.region.start {
            panic!()
        }

        let entry = MapleEntry::new_leaf(value);

        if self.root.is_empty() {
            let elem = self.allocator.alloc_elem().init();
            elem.set(0, range.start, MapleEntry::new_leaf(T::default()));
            elem.set(1, range.end, entry);
            self.root = elem.as_ptr();
            return;
        }

        let v = self.get_value(range.start).unwrap();
        if *v != T::default() {
            if *v == value {
                self.remove_range_end(range.start);
                self.insert_range_end(range.end, entry);
            } else {
                self.insert_range_end(range.end, entry);
            }
        } else {
            self.insert_range_start(range.start);
            self.insert_range_end(range.end, entry);
        }
    }

    fn insert_range_start(&mut self, start: usize) {
        if start == self.region.start {
            return;
        }

        let path = self.search_path(start);

        let (ptr, idx) = path.get_leaf();
        let node = ptr.as_mut();
        if idx != node.get_size() && node.keys[idx] == start {
            // start already inserted, return
            return;
        }

        let entry = MapleEntry::default();

        mtree_insert_at_impl(&mut self.root, start, entry, path, &mut self.allocator);
    }

    fn insert_range_end(&mut self, end: usize, entry: MapleEntry<T>) {
        if end == self.region.end {
            return;
        }

        let path = self.search_path(end);

        let (ptr, idx) = path.get_leaf();
        let node = ptr.as_mut();
        if idx != node.get_size() && node.keys[idx] == end {
            // end already inserted, replace the entry
            node.entries[idx] = entry;
            return;
        }

        mtree_insert_at_impl(&mut self.root, end, entry, path, &mut self.allocator);
    }

    pub fn remove_range(&mut self, range: Range<usize>) {
        self.remove_range_start(range.start);
        self.remove_range_end(range.end);
    }

    fn remove_range_start(&mut self, start: usize) {
        let path = self.search_path(start);

        let (ptr, idx) = path.get_leaf();
        let node = ptr.as_mut();

        if node.keys[idx] != start {
            let v = self.get_value(start).unwrap();
            self.insert(start, *v);
            return;
        }

        let value = self.get_value(start).unwrap();
        if *value != T::default() {
            return;
        }

        mtree_remove_at_impl(&mut self.root, path, &mut self.allocator);
    }

    fn remove_range_end(&mut self, end: usize) {
        let path = self.search_path(end);

        for (ptr, idx) in path.rev() {
            if idx == 15 {
                continue;
            }
            let node = ptr.as_mut();
            if node.keys[idx] != end {
                self.insert(end, T::default());
                return;
            }
            break;
        }

        mtree_remove_at_impl(&mut self.root, path, &mut self.allocator);
    }
}

pub struct MapleNode<T> {
    keys: [usize; 15],
    entries: [MapleEntry<T>; 16],
}

impl<T: 'static> MapleNode<T> {
    pub unsafe fn from_ptr(ptr: usize) -> &'static Self {
        &*(ptr as *const Self)
    }

    pub unsafe fn from_mut_ptr(ptr: usize) -> &'static mut Self {
        &mut *(ptr as *mut Self)
    }

    fn is_leaf(&self) -> bool {
        // all nodes have same type
        self.entries[0].is_leaf()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.get_size() == 15
    }

    #[inline]
    fn as_ptr(&self) -> NodePtr<T> {
        NodePtr::new(self as *const Self as usize)
    }

    /// Get the size of self.keys
    fn get_size(&self) -> usize {
        self.keys.iter().take_while(|key| **key != 0).count()
    }

    /// Find the index of first key that equal or bigger then given key
    /// @return: the index of key (or the last entry)
    fn search_key(&self, key: usize) -> usize {
        let mut lower = 0;
        let mut upper = self.get_size();

        if key > self.keys[upper - 1] {
            return upper;
        }

        let mut m = self.get_size() / 2;

        while m >= lower && m < upper {
            match self.keys[m].cmp(&key) {
                core::cmp::Ordering::Less => {
                    lower = m;
                    m += (upper - m).div_ceil(2);
                }
                core::cmp::Ordering::Equal => return m,
                core::cmp::Ordering::Greater => {
                    upper = m;
                    m -= (m - lower).div_ceil(2);
                }
            }
        }

        return m;
    }

    /// Find the index of first key that bigger then given key
    /// @return: the index of key (or the last entry)
    fn search_end(&self, key: usize) -> usize {
        let mut lower = 0;
        let mut upper = self.get_size();

        if key > self.keys[upper - 1] {
            return upper;
        }

        let mut m = self.get_size() / 2;

        while m >= lower && m < upper {
            match self.keys[m].cmp(&key) {
                core::cmp::Ordering::Greater => {
                    upper = m;
                    m -= (m - lower).div_ceil(2);
                }
                _ => {
                    lower = m;
                    m += (upper - m).div_ceil(2);
                }
            }
        }

        return m;
    }

    fn set(&mut self, idx: usize, key: usize, entry: MapleEntry<T>) {
        self.keys[idx] = key;
        self.entries[idx] = entry;
    }
}

impl<T: Default + Copy + 'static> MapleNode<T> {
    fn init(&mut self) -> &mut Self {
        self.keys = [0; 15];
        self.entries = [MapleEntry::default(); 16];
        self
    }

    pub fn new() -> Self {
        Self {
            keys: [0; 15],
            entries: [MapleEntry::default(); 16],
        }
    }

    fn take(&mut self, idx: usize) -> (usize, MapleEntry<T>) {
        // get key and entry at idx
        let key = self.keys[idx];
        let entry = self.entries[idx];

        // remove key and entry at idx
        self.remove_at(idx);
        (key, entry)
    }

    fn replace_key(&mut self, idx: usize, key: usize) -> usize {
        let old_key = self.keys[idx];
        self.keys[idx] = key;
        old_key
    }

    /// Replace the self.keys[idx] with predecessor key
    fn replace_predecessor_at(&mut self, idx: usize, path: &mut MTreePath<T>) {
        let mut pred_ptr = self.entries[idx].get_child_ptr();
        let mut pred_node = pred_ptr.as_mut();

        // 1. find leaf node
        while !pred_node.is_leaf() {
            path.push(pred_ptr, pred_node.get_size());
            pred_ptr = pred_node.entries[pred_node.get_size()].get_child_ptr();
            pred_node = pred_ptr.as_mut();
        }

        // 2. switch
        let pred_idx = pred_node.get_size() - 1;
        self.keys[idx] = pred_node.keys[pred_idx];
        pred_node.keys[pred_idx] = 0;
    }

    fn insert_at(&mut self, idx: usize, key: usize, entry: MapleEntry<T>) {
        let size = self.get_size();
        self.entries[size + 1] = self.entries[size];
        self.shift_right(idx, size + 1, 1);
        self.keys[idx] = key;
        self.entries[idx] = entry;
    }

    fn remove_at(&mut self, idx: usize) {
        let size = self.get_size();
        self.shift_left(idx, size, 1);
        self.keys[size - 1] = 0;
        self.entries[size - 1] = self.entries[size];
    }

    fn shift_right(&mut self, start: usize, end: usize, shift: usize) {
        for i in (start..(end - shift)).rev() {
            self.keys[i + shift] = self.keys[i];
            self.entries[i + shift] = self.entries[i];
        }
    }

    fn shift_left(&mut self, start: usize, end: usize, shift: usize) {
        for i in start..(end - shift) {
            self.keys[i] = self.keys[i + shift];
            self.entries[i] = self.entries[i + shift];
        }
    }
}

#[derive(Clone, Copy)]
pub enum MapleEntry<T> {
    Child(NodePtr<T>),
    Leaf(T),
}

impl<T> MapleEntry<T> {
    const fn new_leaf(value: T) -> Self {
        Self::Leaf(value)
    }

    fn new_child(ptr: NodePtr<T>) -> Self {
        Self::Child(ptr)
    }

    fn is_leaf(&self) -> bool {
        match self {
            MapleEntry::Child(_) => false,
            MapleEntry::Leaf(_) => true,
        }
    }

    fn get_child_ptr(&self) -> NodePtr<T> {
        match self {
            MapleEntry::Child(ptr) => *ptr,
            _ => panic!(),
        }
    }

    fn get_leaf(&self) -> &T {
        match self {
            MapleEntry::Leaf(v) => v,
            _ => panic!(),
        }
    }
}

impl<T: Default> Default for MapleEntry<T> {
    fn default() -> Self {
        Self::Leaf(T::default())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct NodePtr<T> {
    ptr: usize,
    _phantom: PhantomData<T>,
}

impl<T> NodePtr<T> {
    pub fn as_ref(self) -> &'static MapleNode<T> {
        unsafe { MapleNode::from_ptr(self.ptr) }
    }

    pub fn as_mut(self) -> &'static mut MapleNode<T> {
        unsafe { MapleNode::from_mut_ptr(self.ptr) }
    }

    fn is_empty(&self) -> bool {
        self.ptr == 0
    }

    fn new(v: usize) -> Self {
        Self {
            ptr: v,
            _phantom: Default::default(),
        }
    }

    const fn empty() -> Self {
        Self {
            ptr: 0,
            _phantom: PhantomData,
        }
    }
}

impl<T> Clone for NodePtr<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            _phantom: PhantomData::default(),
        }
    }
}

impl<T> Copy for NodePtr<T> {}

struct MTreePath<T> {
    node_path: [NodePtr<T>; 16],
    idx_path: [usize; 16],
    len: usize,
}

impl<T> MTreePath<T> {
    fn new() -> Self {
        Self {
            node_path: [NodePtr::empty(); 16],
            idx_path: [0; 16],
            len: 0,
        }
    }

    fn push(&mut self, ptr: NodePtr<T>, idx: usize) {
        self.node_path[self.len] = ptr;
        self.idx_path[self.len] = idx;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<(NodePtr<T>, usize)> {
        if self.len == 0 {
            return None;
        } else {
            let res = (self.node_path[self.len - 1], self.idx_path[self.len - 1]);
            self.len -= 1;
            return Some(res);
        }
    }

    fn rev(&self) -> impl Iterator<Item = (NodePtr<T>, usize)> + '_ {
        let mut pos = self.len;
        core::iter::from_fn(move || {
            if pos == 0 {
                None
            } else {
                pos -= 1;
                Some((self.node_path[pos], self.idx_path[pos]))
            }
        })
    }

    fn get_leaf(&self) -> (NodePtr<T>, usize) {
        (self.node_path[self.len - 1], self.idx_path[self.len - 1])
    }
}

struct MTreeOfItem<T> {
    key: usize,
    entry: MapleEntry<T>,
    child_ptr: NodePtr<T>,
}

fn mtree_remove_at_impl<T: Clone + Copy + Default + 'static>(
    root: &mut NodePtr<T>,
    mut path: MTreePath<T>,
    allocator: &mut ArrayAllocator<MapleNode<T>>,
) {
    let (ptr, idx) = path.get_leaf();
    let node = ptr.as_mut();

    if node.is_leaf() {
        node.remove_at(idx);
    } else {
        node.replace_predecessor_at(idx, &mut path);
    }

    mtree_solve_underflow(root, path, allocator);
}

fn mtree_insert_at_impl<T: Clone + Copy + Default + 'static>(
    root: &mut NodePtr<T>,
    mut key: usize,
    mut entry: MapleEntry<T>,
    mut path: MTreePath<T>,
    allocator: &mut ArrayAllocator<MapleNode<T>>,
) {
    let (mut ptr, mut idx) = path.pop().unwrap();
    let mut node = ptr.as_mut();

    loop {
        if node.is_full() {
            // need solve overflow
            let of = mtree_solve_overflow(ptr, idx, key, entry, allocator);

            if let Some(parent) = path.pop() {
                (ptr, idx) = parent;
                node = ptr.as_mut();
                node.entries[idx] = MapleEntry::<T>::new_child(of.child_ptr);
                // insert splited key and entry into parent node
                key = of.key;
                entry = of.entry;
            } else {
                // is root
                let new_root = allocator.alloc_elem().init();
                new_root.set(0, of.key, of.entry);
                new_root.entries[1] = MapleEntry::new_child(of.child_ptr);
                *root = new_root.as_ptr();
                break;
            }
        } else {
            node.insert_at(idx, key, entry);
            break;
        }
    }
}

fn mtree_solve_overflow<T: Clone + Copy + Default + 'static>(
    ptr: NodePtr<T>,
    idx: usize,
    key: usize,
    entry: MapleEntry<T>,
    allocator: &mut ArrayAllocator<MapleNode<T>>,
) -> MTreeOfItem<T> {
    let mid_idx = 7;
    let node = ptr.as_mut();
    let mid_key;
    let mid_entry;

    // 1. select the middle key
    if idx == mid_idx {
        // new key is the middle, no need to replace
        mid_key = key;
        mid_entry = entry;
    } else if idx < mid_idx {
        // new key are smaller than middle key, thus insert before middle
        mid_key = node.keys[mid_idx - 1];
        mid_entry = node.entries[mid_idx - 1];

        node.shift_right(idx, mid_idx - 1, 1);
        node.keys[idx] = key;
        node.entries[idx] = entry;
    } else {
        // new key are bigger than middle key
        mid_key = node.keys[mid_idx];
        mid_entry = node.entries[mid_idx];

        node.shift_left(mid_idx, idx, 1);
        node.keys[idx - 1] = key;
        node.entries[idx - 1] = entry;
    }

    // 2. split the child node at middle (0..=7, 8..=14)
    let new_node = allocator.alloc_elem().init();
    for i in 0..8 {
        new_node.keys[i] = node.keys[i + 7];
        node.keys[i + 7] = 0;

        new_node.entries[i] = node.entries[i + 7];
        node.entries[i + 7] = MapleEntry::default();
    }

    new_node.entries[8] = node.entries[15];
    node.entries[7] = mid_entry;

    // 3. create new entry, which point to current node
    let new_entry = MapleEntry::<T>::Child(node.as_ptr());

    MTreeOfItem {
        key: mid_key,
        entry: new_entry,
        child_ptr: new_node.as_ptr(),
    }
}

fn mtree_solve_underflow<T: Clone + Copy + Default + 'static>(
    root: &mut NodePtr<T>,
    mut path: MTreePath<T>,
    allocator: &mut ArrayAllocator<MapleNode<T>>,
) {
    let median = 7;
    let (mut ptr, _) = path.pop().unwrap();
    let mut node = ptr.as_ref();

    while node.get_size() < median {
        let (p_ptr, p_idx);

        if let Some(p) = path.pop() {
            (p_ptr, p_idx) = p;
        } else {
            // is root
            if node.get_size() == 0 {
                if node.is_leaf() {
                    allocator.add_free_element(node);
                    *root = NodePtr::empty();
                } else {
                    *root = node.entries[0].get_child_ptr();
                    allocator.add_free_element(node);
                }
            }
            return;
        }
        let p_node = p_ptr.as_ref();

        // rotate right?
        if p_idx > 0 {
            // rotate right
            let l_ptr = p_node.entries[p_idx - 1].get_child_ptr();
            if l_ptr.as_ref().get_size() > median {
                mtree_rotate_right(ptr, p_ptr, p_idx, l_ptr);
                return;
            }
        }
        // rotate left?
        if p_idx < p_node.get_size() {
            // rotate left?
            let r_ptr = p_node.entries[p_idx + 1].get_child_ptr();
            if r_ptr.as_ref().get_size() > median {
                mtree_rotate_left(ptr, p_ptr, p_idx, r_ptr);
                return;
            }
        }

        // can't rotate, merge nodes
        if p_idx > 0 {
            mtree_merge(p_idx - 1, p_idx, p_ptr, allocator);
        } else {
            mtree_merge(p_idx, p_idx + 1, p_ptr, allocator);
        }

        node = p_node;
        ptr = p_ptr;
    }
}

fn mtree_rotate_left<T: Clone + Copy + Default + 'static>(
    ptr: NodePtr<T>,
    p_ptr: NodePtr<T>,
    p_idx: usize,
    r_ptr: NodePtr<T>,
) {
    // 1. take first key in the right node
    let r_node = r_ptr.as_mut();
    let (key, entry) = r_node.take(0);

    // 2. replace key in the parent node at idx
    let p_node = p_ptr.as_mut();
    let p_key = p_node.replace_key(p_idx, key);

    // 2. move key in the parent node
    let node = ptr.as_mut();
    let idx = node.get_size();
    node.keys[idx] = p_key;
    node.entries[idx + 1] = entry;
}

fn mtree_rotate_right<T: Clone + Copy + Default + 'static>(
    ptr: NodePtr<T>,
    p_ptr: NodePtr<T>,
    p_idx: usize,
    l_ptr: NodePtr<T>,
) {
    // 1. take last key in the left node
    let l_node = l_ptr.as_mut();
    let l_size = l_node.get_size();
    let key = l_node.keys[l_size - 1];
    l_node.keys[l_size - 1] = 0;

    // 2. replace key in the parent node at idx
    let p_node = p_ptr.as_mut();
    // get p_entry (last entry in l_node's entries)
    let p_entry = l_node.entries[l_size];
    let p_key = p_node.replace_key(p_idx - 1, key);

    // 3. store parent key
    let node = ptr.as_mut();
    node.insert_at(0, p_key, p_entry);
}

fn mtree_merge<T: Clone + Copy + Default + 'static>(
    l_idx: usize,
    r_idx: usize,
    p_ptr: NodePtr<T>,
    allocator: &mut ArrayAllocator<MapleNode<T>>,
) {
    // 1. move keys and entries
    let p_node = p_ptr.as_mut();
    let l_node = p_node.entries[l_idx].get_child_ptr().as_mut();
    let r_node = p_node.entries[r_idx].get_child_ptr().as_mut();

    assert!(l_node.get_size() + r_node.get_size() + 1 <= 15);

    // add parent key into left node
    let p_key = p_node.keys[l_idx];
    l_node.keys[l_node.get_size()] = p_key;

    let start = l_node.get_size();
    for i in 0..r_node.get_size() {
        l_node.keys[start + i] = r_node.keys[i];
        l_node.entries[start + i] = r_node.entries[i];
    }
    //
    l_node.entries[start + r_node.get_size()] = r_node.entries[r_node.get_size()];

    // recycle right node
    allocator.add_free_element(&r_node);
    // update right child in parent
    p_node.entries[r_idx] = p_node.entries[l_idx];
    // remove parent key
    p_node.remove_at(l_idx);
}

#[cfg(test)]
mod test {
    use super::{MapleEntry, MapleNode, MapleTree};
    use crate::array_alloc::ArrayAllocator;

    const LEAF: MapleEntry<usize> = MapleEntry::Leaf(0);

    const NODE: MapleNode<usize> = MapleNode {
        keys: [0; 15],
        entries: [LEAF; 16],
    };

    #[test]
    pub fn mtree_create() {
        static mut MTREE_NODES: [MapleNode<usize>; 256] = [NODE; 256];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..200, allocator);

        tree.insert(10, 1);
        assert_eq!(*tree.get_value(2).unwrap(), 1);
        tree.insert(15, 2);
        assert_eq!(*tree.get_value(11).unwrap(), 2);
        tree.insert(13, 3);
        assert_eq!(*tree.get_value(11).unwrap(), 3);
    }

    #[test]
    pub fn mtree_insert() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        assert_eq!(allocator.get_free_size(), 1024);
        let mut tree = MapleTree::new(0..2048, allocator);

        for i in 1..1024 {
            tree.insert(i, i);
            assert_eq!(*tree.get_value(i).unwrap(), i);
        }
    }

    #[test]
    pub fn mtree_remove() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..1024, allocator);

        for i in 1..1024 {
            tree.insert(i, i);
            tree.remove(i);
            let value = tree.get_value(i);
            if let Some(value) = value {
                assert_eq!(*value, i + 1);
            }
        }

        for i in 1..1024 {
            tree.insert(i, i);
        }

        for i in (1..1024).rev() {
            tree.remove(i);
            let value = tree.get_value(i);
            if let Some(value) = value {
                assert_eq!(*value, 0);
            }
        }

        assert!(tree.root.is_empty());
        assert_eq!(tree.allocator.get_free_size(), 1024);

        tree.insert(10, 10);
        tree.remove(8);
        assert_eq!(*tree.get_value(10).unwrap(), 10);
    }

    #[test]
    pub fn mtree_get_range() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..2048, allocator);

        for i in 1..256 {
            tree.insert(i * 3, i);
        }

        assert_eq!(tree.get_range(2).unwrap().0, 0..3);
        assert_eq!(*tree.get_range(2).unwrap().1, 1);
        assert_eq!(tree.get_range(22).unwrap().0, 21..24);
        assert_eq!(*tree.get_range(21).unwrap().1, 8);
    }

    #[test]
    pub fn mtree_insert_range() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..2048, allocator);

        tree.insert_range(4..10, 10);
        tree.insert_range(10..15, 15);
        tree.insert_range(15..20, 15);

        assert_eq!(tree.get_range(1).unwrap().0, 0..4);
        assert_eq!(tree.get_range(4).unwrap().0, 4..10);
        assert_eq!(tree.get_range(10).unwrap().0, 10..20);
        assert_eq!(tree.get_range(100).unwrap().0, 20..2048);

        assert_eq!(*tree.get_range(15).unwrap().1, 15);
        assert_eq!(*tree.get_range(10).unwrap().1, 15);
    }

    #[test]
    pub fn mtree_insert_range_merge() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..2048, allocator);

        (1..1000).zip((2..1001)).for_each(|(start, end)| {
            tree.insert_range(start..end, 1);
            assert_eq!(tree.get_range(start).unwrap().0, 1..end);
        })
    }

    #[test]
    pub fn mtree_remove_range() {
        static mut MTREE_NODES: [MapleNode<usize>; 1024] = [NODE; 1024];

        let allocator = ArrayAllocator::new(unsafe { &mut MTREE_NODES });
        let mut tree = MapleTree::new(0..2048, allocator);

        tree.insert_range(1..10, 10);
        tree.insert_range(10..15, 15);
        tree.insert_range(15..20, 15);
        tree.remove_range(15..20);

        assert_eq!(tree.get_range(10).unwrap().0, 10..15);

        tree.remove_range(11..13);
        assert_eq!(tree.get_range(13).unwrap(), (13..15, &15));
    }
}
