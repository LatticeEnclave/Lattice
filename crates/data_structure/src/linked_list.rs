use core::ptr::NonNull;

pub struct LinkedList<T> {
    tail: Option<NonNull<Node<T>>>,
    head: Option<NonNull<Node<T>>>,
    len: usize,
}

impl<T> LinkedList<T> {
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

    pub fn rm_node<'a>(&mut self, node: &'a mut Node<T>) -> Option<NonNull<Node<T>>> {
        if self.len == 0{
            return None;
        }

        self.len -= 1;

        let prev = node.prev.map(|mut ptr| unsafe { ptr.as_mut() });
        let next = node.next.map(|mut ptr| unsafe { ptr.as_mut() });

        if let Some(prev) = prev {
            prev.next = node.next;
        } else {
            self.head = node.next;
        }

        if let Some(next) = next {
            next.prev = node.prev;
        } else {
            self.tail = node.prev;
        }

        node.prev = None;
        node.next = None;

        Some(NonNull::from(node))
    }

    pub fn push_node(&mut self, node: &mut Node<T>) {
        if self.len == 0 {
            let ptr = NonNull::from(node);
            self.head = Some(ptr);
            self.tail = Some(ptr);
            self.len += 1;
            return;
        }

        let mut old_tail = self.tail.unwrap();
        node.prev = Some(old_tail);
        let ptr = NonNull::from(node);
        unsafe { old_tail.as_mut().next = Some(ptr) }
        self.tail = Some(ptr);
        self.len += 1;
    }

    pub fn iter(&self) -> impl Iterator<Item = NonNull<Node<T>>> {
        let mut current = self.head;
        core::iter::from_fn(move || unsafe {
            let node = current?;
            current = node.as_ref().next;
            Some(node)
        })
    }
}

pub struct Node<T> {
    pub value: T,
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
}

#[cfg(test)]
mod test {}
