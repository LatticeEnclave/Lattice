pub fn reloc_sm(start: usize, dst: usize, end: usize) {
    let src = start as *const u8;
    let dst = dst as *mut u8;

    unsafe {
        reloc(src, dst, end - start);
    }
}

pub fn reloc_payload(start: usize, dst: usize, end: usize) -> usize {
    let src = start as *const u8;
    let dst = dst as *mut u8;

    unsafe {
        reloc(src, dst, end - start);
    }

    dst as usize
}

unsafe fn reloc(src: *const u8, dst: *mut u8, len: usize) {
    let dst = core::slice::from_raw_parts_mut(dst, len);
    let src = core::slice::from_raw_parts(src, len);
    for i in 0..len {
        dst[i] = src[i];
    }
}
