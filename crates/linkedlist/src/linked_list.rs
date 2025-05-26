use core::ptr::NonNull;

use crate::iter::{Iter, IterMut};

pub struct LinkedList<T> {
    tail: Option<NonNull<Node<T>>>,
    head: Option<NonNull<Node<T>>>,
    len: usize,
}

impl<T: 'static> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            tail: None,
            head: None,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push_node(&mut self, node: &mut Node<T>) {
        if self.is_empty() {
            self.insert_empty_list(node);
            return;
        }

        let mut old_tail = self.tail.unwrap();
        node.prev = Some(old_tail);
        let ptr = NonNull::from(node);
        unsafe { old_tail.as_mut().next = Some(ptr) }
        self.tail = Some(ptr);
        self.len += 1;
    }

    pub fn insert_node_by_addr(&mut self, node: &mut Node<T>) {
        if self.is_empty() {
            self.insert_empty_list(node);
            return;
        }

        let addr = node as *mut Node<T> as usize;
        for n in self.iter_mut() {
            let n_addr = n as *mut Node<T> as usize;
            if n_addr > addr {
                let mut prev = n.prev.unwrap_or(NonNull::from(&mut *n));
                node.prev = Some(prev);
                node.next = Some(NonNull::from(&mut *n));
                let ptr = NonNull::from(node);
                unsafe {
                    prev.as_mut().next = Some(ptr);
                    n.prev = Some(ptr);
                }
                self.len += 1;
                return;
            }
        }
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            current: self.head.clone(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            current: self.head.clone(),
        }
    }

    fn insert_empty_list(&mut self, node: &mut Node<T>) {
        let ptr = NonNull::from(node);
        self.head = Some(ptr);
        self.tail = Some(ptr);
        self.len += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub struct NodePtr<T> {
    ptr: NonNull<Node<T>>,
}

impl<T> NodePtr<T> {
    pub fn new(ptr: NonNull<Node<T>>) -> Self {
        Self { ptr }
    }

    pub fn value(&self) -> &T {
        unsafe { self.ptr.as_ref().value() }
    }

    pub fn value_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut().value_mut() }
    }

    pub fn get_next_or_self(&self) -> NodePtr<T> {
        NodePtr {
            ptr: unsafe { self.ptr.as_ref().next.unwrap_or(self.ptr.clone()) },
        }
    }

    //pub fn get_next_or_self_mut(&mut self) -> Self {
    //    let next = unsafe { self.ptr.as_mut().next };
    //    next.map(|mut n| Self::new(n)).unwrap_or(self)
    //}
}

impl<T> Clone for NodePtr<T> {
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}

pub struct Node<T> {
    value: T,
    pub next: Option<NonNull<Node<T>>>,
    pub prev: Option<NonNull<Node<T>>>,
}

impl<T> Node<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            next: None,
            prev: None,
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn get_next_or_self(&self) -> &Self {
        self.next.map(|n| unsafe { n.as_ref() }).unwrap_or(self)
    }

    pub fn get_next_or_self_mut(&mut self) -> &mut Self {
        self.next.map(|mut n| unsafe { n.as_mut() }).unwrap_or(self)
    }
}

#[cfg(test)]
mod test {}
