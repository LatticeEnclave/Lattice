use crate::{
    kernel::{self, LinuxDriverKernel},
    syscall::{sbi_copy_from_user, sbi_copy_to_user},
};
use alloc::boxed::Box;
use load_module::SymbolTable;
use spin::{Once, RwLock};

// static SYMBOL_TABLE: Once<&'static RwLock<SymbolTable>> = Once::new();

pub fn init_symbol_table(table: &mut SymbolTable) {
    table.add_symbol("memset", memset as usize);
    table.add_symbol("_copy_from_user", ldr_copy_from_user as usize);
    table.add_symbol("_copy_to_user", ldr_copy_to_user as usize);
}

// // 获取全局符号表实例
// pub fn get_symbol_table() -> &'static RwLock<SymbolTable> {
//     if let Some(table) = SYMBOL_TABLE.get() {
//         return table;
//     } else {
//         let mut table = SymbolTable::new();
//         init_symbol_table(&mut table);
//         let ptr = Box::new(RwLock::new(table));
//         let leak_ptr = Box::leak(ptr);
//         SYMBOL_TABLE.call_once(move || leak_ptr);
//         get_symbol_table()
//     }
// }

// // 添加自定义符号到全局表
// pub fn add_symbol(name: &str, address: usize) {
//     let table = get_symbol_table();
//     table.write().add_symbol(name, address);
// }

// 空的memset实现，仅用于符号表
pub fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    // 在真实环境中，这将实际设置内存
    // 但在我们的实现中，它只是一个占位符
    dest
}

pub fn ldr_copy_from_user(to: usize, from: usize, n: usize) -> usize {
    let kernel = unsafe { LinuxDriverKernel::from_sscratch() };
    let (error, value) = sbi_copy_from_user(to, from, n);

    value as usize
}

pub fn ldr_copy_to_user(to: usize, from: usize, n: usize) -> usize {
    let kernel = unsafe { LinuxDriverKernel::from_sscratch() };
    let (error, value) = sbi_copy_to_user(to, from, n);

    value as usize
}

// pub fn kmalloc(size: usize, flag: u32) {
//     let kernel = unsafe { LinuxDriverKernel::from_sscratch() };

//     //todo: ignore the dma access(the physical pages continuity)

// }
