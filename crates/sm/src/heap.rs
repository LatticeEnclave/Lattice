use core::ptr::NonNull;

use data_structure::intrusive_list::{FromList, IntoList, ListHead};
use mempool::Mempool;

pub const CHUNK_SIZE: usize = 0x1000;

pub struct Chunk {
    list: ListHead<Chunk>,
}

impl FromList for Chunk {
    fn from_list<'a>(ptr: NonNull<ListHead<Self>>) -> &'a mut Self {
        // 通过指针偏移获取宿主对象
        unsafe {
            let offset = core::mem::offset_of!(Chunk, list);
            let ptr = ptr.as_ptr() as *mut u8;
            &mut *(ptr.offset(-(offset as isize)) as *mut Chunk)
        }
    }
}

impl IntoList for Chunk {
    fn into_list(&mut self) -> NonNull<ListHead<Self>> {
        unsafe { NonNull::new_unchecked(&mut self.list) }
    }
}

pub struct Heap {
    list: ListHead<Chunk>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            list: ListHead::new()
        }
    }

    pub fn init(&mut self, base: *mut u8, size: usize) {
        self.list.init();
        assert_ne!(base, 0 as *mut u8);
        assert!(
            base as usize % CHUNK_SIZE == 0,
            "Base address must be aligned to 0x1000"
        );
        assert!(
            size % CHUNK_SIZE == 0,
            "Memory size must be a multiple of 0x1000"
        );

        let num_chunks = size / CHUNK_SIZE;
        let mut current_ptr = base;

        unsafe {
            for _ in 0..num_chunks {
                // 将当前内存地址转换为 Chunk 类型
                let chunk = current_ptr as *mut Chunk;
                // 初始化 chunk 的链表节点
                (*chunk).list.init();
                // 插入到链表中
                self.list.push(&mut *chunk);

                current_ptr = current_ptr.add(CHUNK_SIZE);
            }
        }
    }

    #[inline]
    pub fn alloc_mempool(&mut self) -> Option<Mempool> {
        self.list.pop().map(|chunk| {
            Mempool::new(unsafe {
                core::slice::from_raw_parts_mut(chunk as *mut _ as *mut u8, CHUNK_SIZE)
            })
        })
    }

    #[inline]
    pub unsafe fn free_mempool(&mut self, mempool: Mempool) {
        let chunk = mempool.get_start().as_ptr() as *mut Chunk;
        unsafe {
            (*chunk).list.init();
            self.list.push(&mut *chunk);
        }
    }
}
