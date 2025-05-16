use core::ptr::NonNull;

use crate::linked_list::Node;

pub struct Iter<T> {
    pub current: Option<NonNull<Node<T>>>,
}

impl<T: 'static> Iterator for Iter<T> {
    type Item = &'static Node<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        let node = unsafe { current.as_ref() };

        self.current = node.next;

        Some(node)
    }
}

pub struct IterMut<T> {
    pub current: Option<NonNull<Node<T>>>,
}

impl<T: 'static> Iterator for IterMut<T> {
    type Item = &'static mut Node<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut current = self.current?;
        let node = unsafe { current.as_mut() };

        self.current = node.next;

        Some(node)
    }
}
