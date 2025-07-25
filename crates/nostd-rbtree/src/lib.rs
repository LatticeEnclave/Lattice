// This file is modified from https://github.com/tickbh/rbtree-rs.
// Copyright 2017-2018 By tickdream125@hotmail.com.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![no_std]

use core::cmp::Ord;
use core::cmp::Ordering;
use core::fmt::{self, Debug};
use core::iter::IntoIterator;
use core::marker;
use core::mem;
use core::mem::ManuallyDrop;
use core::ops::Index;
use core::ptr;
use core::ptr::NonNull;

pub const fn node_size<K: Ord, V>() -> usize {
    core::mem::size_of::<RBTreeNode<K, V>>()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

/*****************RBTreeNode***************************/
pub struct RBTreeNode<K: Ord, V> {
    color: Color,
    pub left: NodePtr<K, V>,
    pub right: NodePtr<K, V>,
    pub parent: NodePtr<K, V>,
    pub key: K,
    pub value: V,
}

impl<K: Ord, V> RBTreeNode<K, V> {
    #[inline]
    fn pair(&self) -> (K, V) {
        unsafe {
            let key = core::ptr::read(&self.key);
            let value = core::ptr::read(&self.value);
            (key, value)
        }
    }

    #[inline]
    fn init(&mut self, k: K, v: V) {
        self.color = Color::Black;
        self.left = NodePtr::null();
        self.right = NodePtr::null();
        self.parent = NodePtr::null();
        self.key = k;
        self.value = v;
    }
}

impl<K, V> Debug for RBTreeNode<K, V>
where
    K: Ord + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "k:{:?} v:{:?} c:{:?}", self.key, self.value, self.color)
    }
}

union AllocNode<K: Ord, V> {
    next: *mut AllocNode<K, V>,
    node: ManuallyDrop<RBTreeNode<K, V>>,
}

struct NodeList<K: Ord, V> {
    head: *mut AllocNode<K, V>,
    len: usize,
}

impl<K: Ord, V> NodeList<K, V> {
    fn pop(&mut self) -> Option<NonNull<RBTreeNode<K, V>>> {
        unsafe {
            if let Some(ptr) = self.head.as_mut() {
                self.head = ptr.next;
                self.len -= 1;
                NonNull::new(&mut ptr.node as *mut _ as *mut RBTreeNode<K, V>)
            } else {
                None
            }
        }
    }

    fn push(&mut self, node: NonNull<RBTreeNode<K, V>>) {
        unsafe {
            let alloc_node = &mut *(node.as_ptr() as *mut AllocNode<K, V>);
            alloc_node.next = self.head;
            self.head = alloc_node as *mut _;
            self.len += 1;
        }
    }
}

pub struct RBTreeAllocator<K: Ord, V> {
    arena: NonNull<[RBTreeNode<K, V>]>,
    list: NodeList<K, V>,
    _phantom: marker::PhantomData<(K, V)>,
}

impl<K: Ord, V> RBTreeAllocator<K, V> {
    pub fn new(arena: &mut [u8]) -> RBTreeAllocator<K, V> {
        let arena = unsafe { arena.align_to_mut::<RBTreeNode<K, V>>().1 };

        let mut list = NodeList {
            len: 0,
            head: ptr::null_mut(),
        };

        for node in arena.iter_mut() {
            let alloc_node = node as *mut _ as *mut RBTreeNode<_, _>;
            unsafe {
                list.push(NonNull::new_unchecked(alloc_node));
            }
        }

        RBTreeAllocator {
            arena: NonNull::new(arena).unwrap(),
            list,
            _phantom: marker::PhantomData,
        }
    }

    fn alloc_node(&mut self, k: K, v: V) -> Option<NodePtr<K, V>> {
        let mut node = self.list.pop()?;
        unsafe { node.as_mut().init(k, v) };
        Some(NodePtr(node.as_ptr()))
    }

    fn dealloc_node(&mut self, ptr: NodePtr<K, V>) {
        if let Some(ptr) = NonNull::new(ptr.0) {
            self.list.push(ptr);
        }
    }
}

/*****************NodePtr***************************/
#[derive(Debug)]
pub struct NodePtr<K: Ord, V>(pub *mut RBTreeNode<K, V>);

impl<K: Ord, V> Clone for NodePtr<K, V> {
    fn clone(&self) -> NodePtr<K, V> {
        NodePtr(self.0)
    }
}

impl<K: Ord, V> Copy for NodePtr<K, V> {}

impl<K: Ord, V> Ord for NodePtr<K, V> {
    fn cmp(&self, other: &NodePtr<K, V>) -> Ordering {
        unsafe { (*self.0).key.cmp(&(*other.0).key) }
    }
}

impl<K: Ord, V> PartialOrd for NodePtr<K, V> {
    fn partial_cmp(&self, other: &NodePtr<K, V>) -> Option<Ordering> {
        unsafe { Some((*self.0).key.cmp(&(*other.0).key)) }
    }
}

impl<K: Ord, V> PartialEq for NodePtr<K, V> {
    fn eq(&self, other: &NodePtr<K, V>) -> bool {
        self.0 == other.0
    }
}

impl<K: Ord, V> Eq for NodePtr<K, V> {}

impl<K: Ord, V> NodePtr<K, V> {
    // fn new(k: K, v: V) -> NodePtr<K, V> {
    //     let node = RBTreeNode {
    //         color: Color::Black,
    //         left: NodePtr::null(),
    //         right: NodePtr::null(),
    //         parent: NodePtr::null(),
    //         key: k,
    //         value: v,
    //     };
    //     NodePtr(Box::into_raw(Box::new(node)))
    // }

    #[inline]
    fn set_color(&mut self, color: Color) {
        if self.is_null() {
            return;
        }
        unsafe {
            (*self.0).color = color;
        }
    }

    #[inline]
    fn set_red_color(&mut self) {
        self.set_color(Color::Red);
    }

    #[inline]
    pub fn get_key_value(&self) -> Option<(&K, &mut V)> {
        if self.is_null() {
            return None;
        }
        unsafe { Some((&(*self.0).key, &mut (*self.0).value)) }
    }

    #[inline]
    fn set_black_color(&mut self) {
        self.set_color(Color::Black);
    }

    #[inline]
    fn get_color(&self) -> Color {
        if self.is_null() {
            return Color::Black;
        }
        unsafe { (*self.0).color }
    }

    #[inline]
    fn is_red_color(&self) -> bool {
        if self.is_null() {
            return false;
        }
        unsafe { (*self.0).color == Color::Red }
    }

    #[inline]
    fn is_black_color(&self) -> bool {
        if self.is_null() {
            return true;
        }
        unsafe { (*self.0).color == Color::Black }
    }

    #[inline]
    fn is_left_child(&self) -> bool {
        self.parent().left() == *self
    }

    #[inline]
    fn is_right_child(&self) -> bool {
        self.parent().right() == *self
    }

    #[inline]
    fn min_node(self) -> NodePtr<K, V> {
        let mut temp = self.clone();
        while !temp.left().is_null() {
            temp = temp.left();
        }
        return temp;
    }

    #[inline]
    fn max_node(self) -> NodePtr<K, V> {
        let mut temp = self.clone();
        while !temp.right().is_null() {
            temp = temp.right();
        }
        return temp;
    }

    #[inline]
    fn next(self) -> NodePtr<K, V> {
        if !self.right().is_null() {
            self.right().min_node()
        } else {
            let mut temp = self;
            loop {
                if temp.parent().is_null() {
                    return NodePtr::null();
                }
                if temp.is_left_child() {
                    return temp.parent();
                }
                temp = temp.parent();
            }
        }
    }

    #[inline]
    fn prev(self) -> NodePtr<K, V> {
        if !self.left().is_null() {
            self.left().max_node()
        } else {
            let mut temp = self;
            loop {
                if temp.parent().is_null() {
                    return NodePtr::null();
                }
                if temp.is_right_child() {
                    return temp.parent();
                }
                temp = temp.parent();
            }
        }
    }

    #[inline]
    fn set_parent(&mut self, parent: NodePtr<K, V>) {
        if self.is_null() {
            return;
        }
        unsafe { (*self.0).parent = parent }
    }

    #[inline]
    fn set_left(&mut self, left: NodePtr<K, V>) {
        if self.is_null() {
            return;
        }
        unsafe { (*self.0).left = left }
    }

    #[inline]
    fn set_right(&mut self, right: NodePtr<K, V>) {
        if self.is_null() {
            return;
        }
        unsafe { (*self.0).right = right }
    }

    #[inline]
    fn parent(&self) -> NodePtr<K, V> {
        if self.is_null() {
            return NodePtr::null();
        }
        unsafe { (*self.0).parent.clone() }
    }

    #[inline]
    fn left(&self) -> NodePtr<K, V> {
        if self.is_null() {
            return NodePtr::null();
        }
        unsafe { (*self.0).left.clone() }
    }

    #[inline]
    fn right(&self) -> NodePtr<K, V> {
        if self.is_null() {
            return NodePtr::null();
        }
        unsafe { (*self.0).right.clone() }
    }

    #[inline]
    pub fn null() -> NodePtr<K, V> {
        NodePtr(ptr::null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    fn pair(&self) -> (K, V) {
        unsafe { (*self.0).pair() }
    }
}

pub struct RBTree<K: Ord, V> {
    pub root: NodePtr<K, V>,
    pub len: usize,
    allocator: RBTreeAllocator<K, V>,
}

unsafe impl<K: Ord, V> Send for RBTree<K, V> {}

unsafe impl<K: Ord, V> Sync for RBTree<K, V> {}

impl<K, V> Debug for RBTree<K, V>
where
    K: Ord + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// all key be same, but it has multi key, if has multi key, it perhaps no correct
impl<K, V> PartialEq for RBTree<K, V>
where
    K: Eq + Ord,
    V: PartialEq,
{
    fn eq(&self, other: &RBTree<K, V>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

impl<K, V> Eq for RBTree<K, V>
where
    K: Eq + Ord,
    V: Eq,
{
}

impl<'a, K, V> Index<&'a K> for RBTree<K, V>
where
    K: Ord,
{
    type Output = V;

    #[inline]
    fn index(&self, index: &K) -> &V {
        self.get(index).expect("no entry found for key")
    }
}

// impl<K: Ord, V> FromIterator<(K, V)> for RBTree<K, V> {
//     fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> RBTree<K, V> {
//         let mut tree = RBTree::new();
//         tree.extend(iter);
//         tree
//     }
// }

/// RBTree into iter
impl<K: Ord, V> Extend<(K, V)> for RBTree<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        let iter = iter.into_iter();
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

pub struct Keys<'a, K: Ord + 'a, V: 'a> {
    inner: Iter<'a, K, V>,
}

impl<'a, K: Ord, V> Clone for Keys<'a, K, V> {
    fn clone(&self) -> Keys<'a, K, V> {
        Keys {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, K: Ord + Debug, V> fmt::Debug for Keys<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K: Ord, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    #[inline]
    fn next(&mut self) -> Option<&'a K> {
        self.inner.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct Values<'a, K: 'a + Ord, V: 'a> {
    inner: Iter<'a, K, V>,
}

impl<'a, K: Ord, V> Clone for Values<'a, K, V> {
    fn clone(&self) -> Values<'a, K, V> {
        Values {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, K: Ord + Debug, V: Debug> fmt::Debug for Values<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K: Ord, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<&'a V> {
        self.inner.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct ValuesMut<'a, K: 'a + Ord, V: 'a> {
    inner: IterMut<'a, K, V>,
}

impl<'a, K: Ord, V> Clone for ValuesMut<'a, K, V> {
    fn clone(&self) -> ValuesMut<'a, K, V> {
        ValuesMut {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, K: Ord + Debug, V: Debug> fmt::Debug for ValuesMut<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K: Ord, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[inline]
    fn next(&mut self) -> Option<&'a mut V> {
        self.inner.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Convert RBTree to iter, move out the tree.
pub struct IntoIter<K: Ord, V> {
    head: NodePtr<K, V>,
    tail: NodePtr<K, V>,
    len: usize,
}

// Drop all owned pointers if the collection is dropped
impl<K: Ord, V> Drop for IntoIter<K, V> {
    #[inline]
    fn drop(&mut self) {
        for (_, _) in self {}
    }
}

impl<K: Ord, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<(K, V)> {
        if self.len == 0 {
            return None;
        }

        if self.head.is_null() {
            return None;
        }

        let next = self.head.next();
        let (k, v) = unsafe {
            (
                core::ptr::read(&(*self.head.0).key),
                core::ptr::read(&(*self.head.0).value),
            )
        };
        self.head = next;
        self.len -= 1;
        Some((k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<K: Ord, V> DoubleEndedIterator for IntoIter<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(K, V)> {
        if self.len == 0 {
            return None;
        }

        if self.tail.is_null() {
            return None;
        }

        let prev = self.tail.prev();
        // let obj = unsafe { Box::from_raw(self.tail.0) };
        // let (k, v) = obj.pair();
        let (k, v) = self.tail.pair();
        self.tail = prev;
        self.len -= 1;
        Some((k, v))
    }
}

pub struct Iter<'a, K: Ord + 'a, V: 'a> {
    head: NodePtr<K, V>,
    tail: NodePtr<K, V>,
    len: usize,
    _marker: marker::PhantomData<&'a ()>,
}

impl<'a, K: Ord + 'a, V: 'a> Clone for Iter<'a, K, V> {
    fn clone(&self) -> Iter<'a, K, V> {
        Iter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            _marker: self._marker,
        }
    }
}

impl<'a, K: Ord + 'a, V: 'a> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        if self.len == 0 {
            return None;
        }

        if self.head.is_null() {
            return None;
        }

        let (k, v) = unsafe { (&(*self.head.0).key, &(*self.head.0).value) };
        self.head = self.head.next();
        self.len -= 1;
        Some((k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K: Ord + 'a, V: 'a> DoubleEndedIterator for Iter<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(&'a K, &'a V)> {
        // println!("len = {:?}", self.len);
        if self.len == 0 {
            return None;
        }

        let (k, v) = unsafe { (&(*self.tail.0).key, &(*self.tail.0).value) };
        self.tail = self.tail.prev();
        self.len -= 1;
        Some((k, v))
    }
}

pub struct IterMut<'a, K: Ord + 'a, V: 'a> {
    head: NodePtr<K, V>,
    tail: NodePtr<K, V>,
    len: usize,
    _marker: marker::PhantomData<&'a ()>,
}

impl<'a, K: Ord + 'a, V: 'a> Clone for IterMut<'a, K, V> {
    fn clone(&self) -> IterMut<'a, K, V> {
        IterMut {
            head: self.head,
            tail: self.tail,
            len: self.len,
            _marker: self._marker,
        }
    }
}

impl<'a, K: Ord + 'a, V: 'a> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        if self.len == 0 {
            return None;
        }

        if self.head.is_null() {
            return None;
        }

        let (k, v) = unsafe { (&(*self.head.0).key, &mut (*self.head.0).value) };
        self.head = self.head.next();
        self.len -= 1;
        Some((k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K: Ord + 'a, V: 'a> DoubleEndedIterator for IterMut<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(&'a K, &'a mut V)> {
        if self.len == 0 {
            return None;
        }

        if self.tail == self.head {
            return None;
        }

        let (k, v) = unsafe { (&(*self.tail.0).key, &mut (*self.tail.0).value) };
        self.tail = self.tail.prev();
        self.len -= 1;
        Some((k, v))
    }
}

impl<K: Ord, V> IntoIterator for RBTree<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(mut self) -> IntoIter<K, V> {
        let iter = if self.root.is_null() {
            IntoIter {
                head: NodePtr::null(),
                tail: NodePtr::null(),
                len: self.len,
            }
        } else {
            IntoIter {
                head: self.first_child(),
                tail: self.last_child(),
                len: self.len,
            }
        };
        self.fast_clear();
        iter
    }
}

impl<K: Ord, V> RBTree<K, V> {
    /// Creates an empty `RBTree`.
    pub fn new(allocator: RBTreeAllocator<K, V>) -> RBTree<K, V> {
        RBTree {
            root: NodePtr::null(),
            len: 0,
            allocator,
        }
    }

    /// Returns the len of `RBTree`.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the `RBTree` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.root.is_null()
    }

    /*
     * 对红黑树的节点(x)进行左旋转
     *
     * 左旋示意图(对节点x进行左旋)：
     *      px                              px
     *     /                               /
     *    x                               y
     *   /  \      --(左旋)-->           / \                #
     *  lx   y                          x  ry
     *     /   \                       /  \
     *    ly   ry                     lx  ly
     *
     *
     */
    #[inline]
    unsafe fn left_rotate(&mut self, mut node: NodePtr<K, V>) {
        let mut temp = node.right();
        node.set_right(temp.left());

        if !temp.left().is_null() {
            temp.left().set_parent(node.clone());
        }

        temp.set_parent(node.parent());
        if node == self.root {
            self.root = temp.clone();
        } else if node == node.parent().left() {
            node.parent().set_left(temp.clone());
        } else {
            node.parent().set_right(temp.clone());
        }

        temp.set_left(node.clone());
        node.set_parent(temp.clone());
    }

    /*
     * 对红黑树的节点(y)进行右旋转
     *
     * 右旋示意图(对节点y进行左旋)：
     *            py                               py
     *           /                                /
     *          y                                x
     *         /  \      --(右旋)-->            /  \                     #
     *        x   ry                           lx   y
     *       / \                                   / \                   #
     *      lx  rx                                rx  ry
     *
     */
    #[inline]
    unsafe fn right_rotate(&mut self, mut node: NodePtr<K, V>) {
        let mut temp = node.left();
        node.set_left(temp.right());

        if !temp.right().is_null() {
            temp.right().set_parent(node.clone());
        }

        temp.set_parent(node.parent());
        if node == self.root {
            self.root = temp.clone();
        } else if node == node.parent().right() {
            node.parent().set_right(temp.clone());
        } else {
            node.parent().set_left(temp.clone());
        }

        temp.set_right(node.clone());
        node.set_parent(temp.clone());
    }

    #[inline]
    pub fn replace_or_insert(&mut self, k: K, mut v: V) -> Option<V> {
        let node = self.find_node(&k);
        if node.is_null() {
            self.insert(k, v);
            return None;
        }

        unsafe {
            mem::swap(&mut v, &mut (*node.0).value);
        }

        Some(v)
    }

    #[inline]
    unsafe fn insert_fixup(&mut self, mut node: NodePtr<K, V>) {
        let mut parent;
        let mut gparent;

        while node.parent().is_red_color() {
            parent = node.parent();
            gparent = parent.parent();
            //若“父节点”是“祖父节点的左孩子”
            if parent == gparent.left() {
                // Case 1条件：叔叔节点是红色
                let mut uncle = gparent.right();
                if !uncle.is_null() && uncle.is_red_color() {
                    uncle.set_black_color();
                    parent.set_black_color();
                    gparent.set_red_color();
                    node = gparent;
                    continue;
                }

                // Case 2条件：叔叔是黑色，且当前节点是右孩子
                if parent.right() == node {
                    self.left_rotate(parent);
                    let temp = parent;
                    parent = node;
                    node = temp;
                }

                // Case 3条件：叔叔是黑色，且当前节点是左孩子。
                parent.set_black_color();
                gparent.set_red_color();
                self.right_rotate(gparent);
            } else {
                // Case 1条件：叔叔节点是红色
                let mut uncle = gparent.left();
                if !uncle.is_null() && uncle.is_red_color() {
                    uncle.set_black_color();
                    parent.set_black_color();
                    gparent.set_red_color();
                    node = gparent;
                    continue;
                }

                // Case 2条件：叔叔是黑色，且当前节点是右孩子
                if parent.left() == node {
                    self.right_rotate(parent);
                    let temp = parent;
                    parent = node;
                    node = temp;
                }

                // Case 3条件：叔叔是黑色，且当前节点是左孩子。
                parent.set_black_color();
                gparent.set_red_color();
                self.left_rotate(gparent);
            }
        }
        self.root.set_black_color();
    }

    #[inline]
    pub fn insert(&mut self, k: K, v: V) {
        self.len += 1;
        // let mut node = NodePtr::new(k, v);
        let mut node = self
            .allocator
            .alloc_node(k, v)
            .expect("Allocing RBTree node panic");
        let mut y = NodePtr::null();
        let mut x = self.root;

        while !x.is_null() {
            y = x;
            match node.cmp(&&mut x) {
                Ordering::Less => {
                    x = x.left();
                }
                _ => {
                    x = x.right();
                }
            };
        }
        node.set_parent(y);

        if y.is_null() {
            self.root = node;
        } else {
            match node.cmp(&&mut y) {
                Ordering::Less => {
                    y.set_left(node);
                }
                _ => {
                    y.set_right(node);
                }
            };
        }

        node.set_red_color();
        unsafe {
            self.insert_fixup(node);
        }
    }

    #[inline]
    fn find_node(&self, k: &K) -> NodePtr<K, V> {
        if self.root.is_null() {
            return NodePtr::null();
        }
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => &mut (*temp.0).right,
                    Ordering::Equal => return *temp,
                };
                if next.is_null() {
                    break;
                }
                temp = next;
            }
        }
        NodePtr::null()
    }

    #[inline]
    fn first_child(&self) -> NodePtr<K, V> {
        if self.root.is_null() {
            NodePtr::null()
        } else {
            let mut temp = self.root;
            while !temp.left().is_null() {
                temp = temp.left();
            }
            return temp;
        }
    }

    #[inline]
    fn last_child(&self) -> NodePtr<K, V> {
        if self.root.is_null() {
            NodePtr::null()
        } else {
            let mut temp = self.root;
            while !temp.right().is_null() {
                temp = temp.right();
            }
            return temp;
        }
    }

    pub fn get_key_value(&self, k: &K) -> Option<(&K, &V)> {
        let node = self.find_node(k);
        if node.is_null() {
            return None;
        }
        unsafe { Some((&(*node.0).key, &mut (*node.0).value)) }
    }

    pub fn get_prev_or_equal_node(&self, k: &K) -> Option<NodePtr<K, V>> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => {
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

    pub fn get_prev_or_equal(&self, k: &K) -> Option<(&K, &V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => {
                        res = temp;
                        &mut (*temp.0).right
                    }
                    Ordering::Equal => return Some((&(*temp.0).key, &(*temp.0).value)),
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
            return unsafe { Some((&(*res.0).key, &(*res.0).value)) };
        }
    }

    pub fn get_prev_or_equal_mut(&self, k: &K) -> Option<(&K, &mut V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => {
                        res = temp;
                        &mut (*temp.0).right
                    }
                    Ordering::Equal => return Some((&(*temp.0).key, &mut (*temp.0).value)),
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
            return unsafe { Some((&(*res.0).key, &mut (*res.0).value)) };
        }
    }

    pub fn get_prev(&self, k: &K) -> Option<(&K, &V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less | Ordering::Equal => &mut (*temp.0).left,
                    Ordering::Greater => {
                        res = temp;
                        &mut (*temp.0).right
                    }
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
            return unsafe { Some((&(*res.0).key, &(*res.0).value)) };
        }
    }

    pub fn get_prev_node(&self, k: &K) -> Option<NodePtr<K, V>> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less | Ordering::Equal => &mut (*temp.0).left,
                    Ordering::Greater => {
                        res = temp;
                        &mut (*temp.0).right
                    } // Ordering::Equal => &mut (*temp.0).left,
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

    pub fn get_prev_mut(&self, k: &K) -> Option<(&K, &mut V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => &mut (*temp.0).left,
                    Ordering::Greater => {
                        res = temp;
                        &mut (*temp.0).right
                    }
                    Ordering::Equal => return Some((&(*temp.0).key, &mut (*temp.0).value)),
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
            return unsafe { Some((&(*res.0).key, &mut (*res.0).value)) };
        }
    }

    pub fn get_next(&self, k: &K) -> Option<(&K, &V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => {
                        res = temp;
                        &mut (*temp.0).left
                    }
                    Ordering::Greater | Ordering::Equal => &mut (*temp.0).right,
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
            return unsafe { Some((&(*res.0).key, &(*res.0).value)) };
        }
    }

    pub fn get_next_node(&self, k: &K) -> Option<NodePtr<K, V>> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => {
                        res = temp;
                        &mut (*temp.0).left
                    }
                    Ordering::Greater | Ordering::Equal => &mut (*temp.0).right,
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

    pub fn get_next_mut(&self, k: &K) -> Option<(&K, &mut V)> {
        if self.root.is_null() {
            return None;
        }
        let mut res = &NodePtr::null();
        let mut temp = &self.root;
        unsafe {
            loop {
                let next = match k.cmp(&(*temp.0).key) {
                    Ordering::Less => {
                        res = temp;
                        &mut (*temp.0).left
                    }
                    Ordering::Greater | Ordering::Equal => &mut (*temp.0).right,
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
            return unsafe { Some((&(*res.0).key, &mut (*res.0).value)) };
        }
    }

    #[inline]
    pub fn get_first(&self) -> Option<(&K, &V)> {
        let first = self.first_child();
        if first.is_null() {
            return None;
        }
        unsafe { Some((&(*first.0).key, &(*first.0).value)) }
    }

    #[inline]
    pub fn get_last(&self) -> Option<(&K, &V)> {
        let last = self.last_child();
        if last.is_null() {
            return None;
        }
        unsafe { Some((&(*last.0).key, &(*last.0).value)) }
    }

    #[inline]
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        let first = self.first_child();
        if first.is_null() {
            return None;
        }
        unsafe { Some(self.delete(first)) }
    }

    #[inline]
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        let last = self.last_child();
        if last.is_null() {
            return None;
        }
        unsafe { Some(self.delete(last)) }
    }

    #[inline]
    pub fn get_first_mut(&mut self) -> Option<(&K, &mut V)> {
        let first = self.first_child();
        if first.is_null() {
            return None;
        }
        unsafe { Some((&(*first.0).key, &mut (*first.0).value)) }
    }

    #[inline]
    pub fn get_last_mut(&mut self) -> Option<(&K, &mut V)> {
        let last = self.last_child();
        if last.is_null() {
            return None;
        }
        unsafe { Some((&(*last.0).key, &mut (*last.0).value)) }
    }

    #[inline]
    pub fn get(&self, k: &K) -> Option<&V> {
        let node = self.find_node(k);
        if node.is_null() {
            return None;
        }

        unsafe { Some(&(*node.0).value) }
    }

    #[inline]
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let node = self.find_node(k);
        if node.is_null() {
            return None;
        }

        unsafe { Some(&mut (*node.0).value) }
    }

    #[inline]
    pub fn contains_key(&self, k: &K) -> bool {
        let node = self.find_node(k);
        if node.is_null() {
            return false;
        }
        true
    }

    /// Empties the `RBTree` without freeing objects in it.
    #[inline]
    fn fast_clear(&mut self) {
        self.root = NodePtr::null();
        self.len = 0;
    }

    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let node = self.find_node(k);
        if node.is_null() {
            return None;
        }

        let v = unsafe { Some(self.delete(node).1) };
        self.allocator.dealloc_node(node);
        v
    }

    #[inline]
    unsafe fn delete_fixup(&mut self, mut node: NodePtr<K, V>, mut parent: NodePtr<K, V>) {
        let mut other;
        while node != self.root && node.is_black_color() {
            if parent.left() == node {
                other = parent.right();
                //x的兄弟w是红色的
                if other.is_red_color() {
                    other.set_black_color();
                    parent.set_red_color();
                    self.left_rotate(parent);
                    other = parent.right();
                }

                //x的兄弟w是黑色，且w的俩个孩子也都是黑色的
                if other.left().is_black_color() && other.right().is_black_color() {
                    other.set_red_color();
                    node = parent;
                    parent = node.parent();
                } else {
                    //x的兄弟w是黑色的，并且w的左孩子是红色，右孩子为黑色。
                    if other.right().is_black_color() {
                        other.left().set_black_color();
                        other.set_red_color();
                        self.right_rotate(other);
                        other = parent.right();
                    }
                    //x的兄弟w是黑色的；并且w的右孩子是红色的，左孩子任意颜色。
                    other.set_color(parent.get_color());
                    parent.set_black_color();
                    other.right().set_black_color();
                    self.left_rotate(parent);
                    node = self.root;
                    break;
                }
            } else {
                other = parent.left();
                //x的兄弟w是红色的
                if other.is_red_color() {
                    other.set_black_color();
                    parent.set_red_color();
                    self.right_rotate(parent);
                    other = parent.left();
                }

                //x的兄弟w是黑色，且w的俩个孩子也都是黑色的
                if other.left().is_black_color() && other.right().is_black_color() {
                    other.set_red_color();
                    node = parent;
                    parent = node.parent();
                } else {
                    //x的兄弟w是黑色的，并且w的左孩子是红色，右孩子为黑色。
                    if other.left().is_black_color() {
                        other.right().set_black_color();
                        other.set_red_color();
                        self.left_rotate(other);
                        other = parent.left();
                    }
                    //x的兄弟w是黑色的；并且w的右孩子是红色的，左孩子任意颜色。
                    other.set_color(parent.get_color());
                    parent.set_black_color();
                    other.left().set_black_color();
                    self.right_rotate(parent);
                    node = self.root;
                    break;
                }
            }
        }

        node.set_black_color();
    }

    #[inline]
    unsafe fn delete(&mut self, node: NodePtr<K, V>) -> (K, V) {
        let mut child;
        let mut parent;
        let color;

        self.len -= 1;
        // 被删除节点的"左右孩子都不为空"的情况。
        if !node.left().is_null() && !node.right().is_null() {
            // 被删节点的后继节点。(称为"取代节点")
            // 用它来取代"被删节点"的位置，然后再将"被删节点"去掉。
            let mut replace = node.right().min_node();
            if node == self.root {
                self.root = replace;
            } else {
                if node.parent().left() == node {
                    node.parent().set_left(replace);
                } else {
                    node.parent().set_right(replace);
                }
            }

            // child是"取代节点"的右孩子，也是需要"调整的节点"。
            // "取代节点"肯定不存在左孩子！因为它是一个后继节点。
            child = replace.right();
            parent = replace.parent();
            color = replace.get_color();
            if parent == node {
                parent = replace;
            } else {
                if !child.is_null() {
                    child.set_parent(parent);
                }
                parent.set_left(child);
                replace.set_right(node.right());
                node.right().set_parent(replace);
            }

            replace.set_parent(node.parent());
            replace.set_color(node.get_color());
            replace.set_left(node.left());
            node.left().set_parent(replace);

            if color == Color::Black {
                self.delete_fixup(child, parent);
            }

            // let obj = Box::from_raw(node.0);
            return node.pair();
        }

        if !node.left().is_null() {
            child = node.left();
        } else {
            child = node.right();
        }

        parent = node.parent();
        color = node.get_color();
        if !child.is_null() {
            child.set_parent(parent);
        }

        if self.root == node {
            self.root = child
        } else {
            if parent.left() == node {
                parent.set_left(child);
            } else {
                parent.set_right(child);
            }
        }

        if color == Color::Black {
            self.delete_fixup(child, parent);
        }

        // let obj = Box::from_raw(node.0);
        return node.pair();
    }

    /// Return the keys iter
    #[inline]
    pub fn keys(&self) -> Keys<K, V> {
        Keys { inner: self.iter() }
    }

    /// Return the value iter
    #[inline]
    pub fn values(&self) -> Values<K, V> {
        Values { inner: self.iter() }
    }

    /// Return the value iter mut
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut {
            inner: self.iter_mut(),
        }
    }

    /// Return the key and value iter
    #[inline]
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            head: self.first_child(),
            tail: self.last_child(),
            len: self.len,
            _marker: marker::PhantomData,
        }
    }

    /// Return the key and mut value iter
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            head: self.first_child(),
            tail: self.last_child(),
            len: self.len,
            _marker: marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::{RBTreeAllocator, RBTreeNode};

    use super::RBTree;

    fn create_allocator<K: Ord, V>() -> RBTreeAllocator<K, V> {
        static mut ARENA: [u8; 1024000] = [0; 1024000];

        unsafe { RBTreeAllocator::new(&mut ARENA) }
    }

    #[test]
    fn test_insert() {
        let mut m = RBTree::new(create_allocator());
        assert_eq!(m.len(), 0);
        m.insert(1, 2);
        assert_eq!(m.len(), 1);
        m.insert(2, 4);
        assert_eq!(m.len(), 2);
        m.insert(2, 6);
        assert_eq!(m.len(), 3);
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&2).unwrap(), 4);
        assert_eq!(*m.get(&2).unwrap(), 4);
    }

    #[test]
    fn test_replace() {
        let mut m = RBTree::new(create_allocator());
        assert_eq!(m.len(), 0);
        m.insert(2, 4);
        assert_eq!(m.len(), 1);
        assert_eq!(m.replace_or_insert(2, 6).unwrap(), 4);
        assert_eq!(m.len(), 1);
        assert_eq!(*m.get(&2).unwrap(), 6);
    }

    // #[test]
    // fn test_clone() {
    //     let mut m = RBTree::new(create_allocator());
    //     assert_eq!(m.len(), 0);
    //     m.insert(1, 2);
    //     assert_eq!(m.len(), 1);
    //     m.insert(2, 4);
    //     assert_eq!(m.len(), 2);
    //     let m2 = m.clone();
    //     m.clear();
    //     assert_eq!(*m2.get(&1).unwrap(), 2);
    //     assert_eq!(*m2.get(&2).unwrap(), 4);
    //     assert_eq!(m2.len(), 2);
    // }

    #[test]
    fn test_empty_remove() {
        let mut m: RBTree<isize, bool> = RBTree::new(create_allocator());
        assert_eq!(m.remove(&0), None);
    }

    #[test]
    fn test_empty_iter() {
        let mut m: RBTree<isize, bool> = RBTree::new(create_allocator());
        assert_eq!(m.iter().next(), None);
        assert_eq!(m.iter_mut().next(), None);
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        assert_eq!(m.into_iter().next(), None);
    }

    #[test]
    fn test_lots_of_insertions() {
        let mut m = RBTree::new(create_allocator());

        // Try this a few times to make sure we never screw up the hashmap's
        // internal state.
        for _ in 0..10 {
            assert!(m.is_empty());

            for i in 1..101 {
                m.insert(i, i);

                for j in 1..i + 1 {
                    let r = m.get(&j);
                    assert_eq!(r, Some(&j));
                }

                for j in i + 1..101 {
                    let r = m.get(&j);
                    assert_eq!(r, None);
                }
            }

            for i in 101..201 {
                assert!(!m.contains_key(&i));
            }

            // remove forwards
            for i in 1..101 {
                assert!(m.remove(&i).is_some());

                for j in 1..i + 1 {
                    assert!(!m.contains_key(&j));
                }

                for j in i + 1..101 {
                    assert!(m.contains_key(&j));
                }
            }

            for i in 1..101 {
                assert!(!m.contains_key(&i));
            }

            for i in 1..101 {
                m.insert(i, i);
            }

            // remove backwards
            for i in (1..101).rev() {
                assert!(m.remove(&i).is_some());

                for j in i..101 {
                    assert!(!m.contains_key(&j));
                }

                for j in 1..i {
                    assert!(m.contains_key(&j));
                }
            }
        }
    }

    #[test]
    fn test_find_mut() {
        let mut m = RBTree::new(create_allocator());
        m.insert(1, 12);
        m.insert(2, 8);
        m.insert(5, 14);
        let new = 100;
        match m.get_mut(&5) {
            None => panic!(),
            Some(x) => *x = new,
        }
        assert_eq!(m.get(&5), Some(&new));
    }

    #[test]
    fn test_remove() {
        let mut m = RBTree::new(create_allocator());
        let size = m.allocator.list.len;
        m.insert(1, 2);
        assert_eq!(*m.get(&1).unwrap(), 2);
        m.insert(5, 3);
        assert_eq!(*m.get(&5).unwrap(), 3);
        m.insert(9, 4);
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert_eq!(m.remove(&1).unwrap(), 2);
        assert_eq!(m.remove(&5).unwrap(), 3);
        assert_eq!(m.remove(&9).unwrap(), 4);
        assert_eq!(m.len(), 0);
        assert_eq!(m.allocator.list.len, size);
    }

    #[test]
    fn test_is_empty() {
        let mut m = RBTree::new(create_allocator());
        m.insert(1, 2);
        assert!(!m.is_empty());
        assert!(m.remove(&1).is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn test_pop() {
        let mut m = RBTree::new(create_allocator());
        m.insert(2, 4);
        m.insert(1, 2);
        m.insert(3, 6);
        assert_eq!(m.len(), 3);
        assert_eq!(m.pop_first(), Some((1, 2)));
        assert_eq!(m.len(), 2);
        assert_eq!(m.pop_last(), Some((3, 6)));
        assert_eq!(m.len(), 1);
        assert_eq!(m.get_first(), Some((&2, &4)));
        assert_eq!(m.get_last(), Some((&2, &4)));
    }

    #[test]
    fn test_iterate() {
        let mut m = RBTree::new(create_allocator());
        for i in 0..32 {
            m.insert(i, i * 2);
        }
        assert_eq!(m.len(), 32);

        let mut observed: u32 = 0;

        for (k, v) in m.iter() {
            assert_eq!(*v, *k * 2);
            observed |= 1 << *k;
        }
        assert_eq!(observed, 0xFFFF_FFFF);
    }

    #[test]
    fn test_keys() {
        let vec = std::vec![(1, 'a'), (2, 'b'), (3, 'c')];
        // let map: RBTree<_, _> = vec.into_iter().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(vec.into_iter());
        let keys: std::vec::Vec<_> = map.keys().cloned().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn test_values() {
        let vec = std::vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let mut map = RBTree::new(create_allocator());
        map.extend(vec.into_iter());
        // let map: RBTree<_, _> = vec.into_iter().collect();
        let values: std::vec::Vec<_> = map.values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn test_values_mut() {
        let vec = std::vec![(1, 1), (2, 2), (3, 3)];
        // let mut map: RBTree<_, _> = vec.into_iter().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(vec.into_iter());
        for value in map.values_mut() {
            *value = (*value) * 2
        }
        let values: std::vec::Vec<_> = map.values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }

    #[test]
    fn test_find() {
        let mut m = RBTree::new(create_allocator());
        assert!(m.get(&1).is_none());
        m.insert(1, 2);
        match m.get(&1) {
            None => panic!(),
            Some(v) => assert_eq!(*v, 2),
        }
    }

    #[test]
    fn test_eq() {
        let mut m1 = RBTree::new(create_allocator());
        m1.insert(1, 2);
        m1.insert(2, 3);
        m1.insert(3, 4);

        let mut m2 = RBTree::new(create_allocator());
        m2.insert(1, 2);
        m2.insert(2, 3);

        assert!(m1 != m2);

        m2.insert(3, 4);

        assert_eq!(m1, m2);
    }

    #[test]
    fn test_show() {
        let mut map = RBTree::new(create_allocator());
        let empty: RBTree<i32, i32> = RBTree::new(create_allocator());

        map.insert(1, 2);
        map.insert(3, 4);

        let map_str = std::format!("{:?}", map);

        assert!(map_str == "{1: 2, 3: 4}" || map_str == "{3: 4, 1: 2}");
        assert_eq!(std::format!("{:?}", empty), "{}");
    }

    #[test]
    fn test_from_iter() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        // let map: RBTree<_, _> = xs.iter().cloned().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(xs.iter().cloned());

        for &(k, v) in &xs {
            assert_eq!(map.get(&k), Some(&v));
        }
    }

    #[test]
    fn test_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        // let map: RBTree<_, _> = xs.iter().cloned().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(xs.iter().cloned());

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        // let map: RBTree<_, _> = xs.iter().cloned().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(xs.iter().cloned());

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.count(), 3);
    }

    #[test]
    fn test_mut_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        // let mut map: RBTree<_, _> = xs.iter().cloned().collect();
        let mut map = RBTree::new(create_allocator());
        map.extend(xs.iter().cloned());

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_mut_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let mut map = RBTree::new(create_allocator());
        map.extend(xs.iter().cloned());

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.count(), 3);
    }

    #[test]
    fn test_index() {
        let mut map = RBTree::new(create_allocator());

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        assert_eq!(map[&2], 1);
    }

    #[test]
    #[should_panic]
    fn test_index_nonexistent() {
        let mut map = RBTree::new(create_allocator());

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        map[&4];
    }

    #[test]
    fn test_rev_iter() {
        let mut a = RBTree::new(create_allocator());
        a.insert(1, 1);
        a.insert(2, 2);
        a.insert(3, 3);

        assert_eq!(a.len(), 3);
        let mut cache = std::vec![];
        for e in a.iter().rev() {
            cache.push(e.0.clone());
        }
        assert_eq!(&cache, &std::vec![3, 2, 1]);
    }

    #[test]
    fn test_get_prev_or_equal() {
        let mut a = RBTree::new(create_allocator());
        a.insert(1, 1);
        a.insert(2, 2);
        a.insert(4, 4);

        assert_eq!(a.get_prev_or_equal(&0), None);
        assert_eq!(a.get_prev_or_equal(&3), Some((&2, &2)));
        assert_eq!(a.get_prev_or_equal(&2), Some((&2, &2)));
        assert_eq!(a.get_prev_or_equal(&5), Some((&4, &4)));
    }

    #[test]
    fn test_get_prev() {
        let mut a = RBTree::new(create_allocator());
        a.insert(1, 1);
        a.insert(2, 2);
        a.insert(4, 4);

        assert_eq!(a.get_prev(&0), None);
        assert_eq!(a.get_prev(&3), Some((&2, &2)));
        assert_eq!(a.get_prev(&2), Some((&1, &1)));
        assert_eq!(a.get_prev(&5), Some((&4, &4)));
    }

    #[test]
    fn test_get_next() {
        let mut a = RBTree::new(create_allocator());
        a.insert(1, 1);
        a.insert(2, 2);
        a.insert(4, 4);

        assert_eq!(a.get_next(&3), Some((&4, &4)));
        assert_eq!(a.get_next(&2), Some((&4, &4)));
        assert_eq!(a.get_next(&5), None);
    }
}
