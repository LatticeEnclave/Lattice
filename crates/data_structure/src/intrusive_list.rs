use core::{marker::PhantomData, ptr::NonNull};

pub trait FromList: Sized {
    fn from_list<'a>(ptr: NonNull<ListHead<Self>>) -> &'a mut Self;
}

pub trait IntoList: Sized {
    fn into_list(&mut self) -> NonNull<ListHead<Self>>;
}

pub struct ListHead<T> {
    pub next: NonNull<ListHead<T>>,
    pub prev: NonNull<ListHead<T>>,
    _phantom: PhantomData<T>,
}

impl<T> ListHead<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            next: NonNull::dangling(),
            prev: NonNull::dangling(),
            _phantom: PhantomData::default(),
        }
    }

    #[inline]
    pub fn init(&mut self) {
        self.clear();
    }

    #[inline]
    fn clear(&mut self) {
        self.next = unsafe { NonNull::new_unchecked(self) };
        self.prev = unsafe { NonNull::new_unchecked(self) };
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.next == NonNull::new_unchecked(self as *const _ as *mut _) }
    }
}

impl<T> ListHead<T>
where
    T: IntoList,
{
    #[inline]
    pub unsafe fn push(&mut self, val: &mut T) {
        let mut ptr = val.into_list();
        unsafe {
            ptr.as_mut().prev = self.prev;
            ptr.as_mut().next = self.prev.as_ref().next;
            self.prev.as_mut().next = ptr;
            ptr.as_mut().next.as_mut().prev = ptr;
        }
    }
}

impl<T> ListHead<T>
where
    T: FromList + 'static,
{
    #[inline]
    pub fn pop(&mut self) -> Option<&mut T> {
        // empty
        if self.is_empty() {
            return None;
        }

        let mut item = self.prev;
        unsafe {
            item.as_mut().prev.as_mut().next = item.as_ref().next;
            item.as_mut().next.as_mut().prev = item.as_ref().prev;
            item.as_mut().clear();
        }

        Some(FromList::from_list(item))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::intrusive_list::ListHead;

    // 定义一个包含链表头的对象结构体
    struct MyObject {
        list_head: ListHead<MyObject>,
        value: i32,
    }

    // 实现 IntoList 和 FromList trait
    impl IntoList for MyObject {
        fn into_list(&mut self) -> NonNull<ListHead<MyObject>> {
            unsafe { NonNull::new_unchecked(&mut self.list_head) }
        }
    }

    impl FromList for MyObject {
        fn from_list<'a>(ptr: NonNull<ListHead<MyObject>>) -> &'a mut Self {
            // 通过指针偏移获取宿主对象
            unsafe {
                let offset = core::mem::offset_of!(MyObject, list_head);
                let ptr = ptr.as_ptr() as *mut u8;
                &mut *(ptr.offset(-(offset as isize)) as *mut MyObject)
            }
        }
    }

    // 测试函数
    #[test]
    fn test_intrusive_list() {
        unsafe {
            // 初始化两个对象
            let mut obj1 = MyObject {
                list_head: ListHead::new(),
                value: 42,
            };

            let mut obj2 = MyObject {
                list_head: ListHead::new(),
                value: 99,
            };

            // 初始化链表头
            let mut head = ListHead::new();
            head.init();

            // 插入对象
            head.push(&mut obj1);
            head.push(&mut obj2);

            // 弹出并验证
            if let Some(popped) = head.pop() {
                assert_eq!(popped.value, 99);
            }

            if let Some(popped) = head.pop() {
                assert_eq!(popped.value, 42);
            }

            // 确保链表为空
            assert!(head.is_empty());
        }
    }
}
