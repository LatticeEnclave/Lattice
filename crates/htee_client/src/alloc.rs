use std::ptr;

use crate::{
    ctl::{teectl_alloc, teectl_free},
    page::{Page, HUGE_PAGE_SIZE},
};

#[macro_export]
macro_rules! align_up {
    ($v: expr, $a: expr) => {
        ((($v) + (($a) - 1)) & !(($a) - 1))
    };
}

pub fn alloc_pages(size: usize, hugepage: bool) -> &'static mut [Page] {
    println!("alloc size: {:#x}", size);
    alloc_pages_from_kernel(size)
}

pub fn free_pages(pages: &mut [Page]) {
    let page_num = pages.len();
    let ptr = pages.as_ptr() as *mut Page;
    teectl_free(ptr as usize, page_num * 0x1000).unwrap();
}

pub fn alloc_normall_pages(size: usize) -> &'static mut [Page] {
    let page_num = size.div_ceil(0x1000) + 1;

    let mut pages = Vec::with_capacity(page_num);

    // walk all page to trigger page fault
    for i in 0..page_num {
        let mut page = Page::new();
        page.set_value(i);
        pages.push(page);
    }

    // never drop
    let pages = Box::new(pages).leak();

    pages
}

pub fn alloc_huge_page(size: usize) -> &'static mut [Page] {
    let num_huge_pages = (size + HUGE_PAGE_SIZE - 1) / HUGE_PAGE_SIZE;
    let total_size = num_huge_pages * HUGE_PAGE_SIZE;

    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            total_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB | libc::MAP_HUGE_2MB,
            -1,
            0,
        )
    };

    println!("huge page mmap: {:#x}", ptr as usize);
    println!("total size: {:#x}", total_size);

    if ptr == libc::MAP_FAILED {
        panic!("mmap failed");
    }

    let pages =
        unsafe { Vec::from_raw_parts(ptr as *mut Page, total_size / 0x1000, total_size / 0x1000) };

    // never drop
    let pages = Box::new(pages).leak();

    pages
        .iter_mut()
        .enumerate()
        .for_each(|(i, p)| p.set_value(i));

    pages
}

pub fn alloc_pages_from_kernel(size: usize) -> &'static mut [Page] {
    let addr = teectl_alloc(size).unwrap();

    let pages = unsafe { Vec::from_raw_parts(addr as *mut Page, size / 0x1000, size / 0x1000) };

    // never drop
    let pages = Box::new(pages).leak();

    pages
        .iter_mut()
        .enumerate()
        .for_each(|(i, p)| p.set_value(i));

    pages
}
