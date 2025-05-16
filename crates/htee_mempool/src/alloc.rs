use crate::map::PoolBitMap;

pub struct Allocator;

impl Allocator {
    pub fn alloc(num: usize, bitmap: &mut PoolBitMap) -> Option<usize> {
        let mut index = 0;
        let mut count = 0;
        for i in 0..bitmap.len {
            if bitmap.test(i) {
                count = 0;
                continue;
            }

            count += 1;
            if count == num {
                index = i - num + 1;
                break;
            }
        }

        if count < num {
            return None;
        }

        for i in index..index + num {
            bitmap.set(i);
        }

        Some(index)
    }
}
