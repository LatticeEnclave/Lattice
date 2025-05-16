use alloc::{collections::btree_map::BTreeMap, string::String};

pub struct SymbolTable {
    symbols: BTreeMap<String, usize>,
}

impl SymbolTable {
    // 创建新的符号表
    pub fn new() -> Self {
        let symbols = BTreeMap::new();
        Self { symbols }
    }

    // 添加符号
    pub fn add_symbol(&mut self, name: &str, address: usize) {
        self.symbols.insert(String::from(name), address);
    }

    // 获取符号地址
    pub fn get_symbol(&self, name: &str) -> Option<usize> {
        self.symbols.get(name).copied()
    }

    pub fn iter_symbols(&self) -> impl Iterator<Item = (&String, &usize)> {
        self.symbols.iter()
    }
}
