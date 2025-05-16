use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub memory: Memory,
    pub module: Option<Vec<Module>>,
    pub runtime: Runtime,
    pub binary: Option<Binary>,
    pub driver: Option<Driver>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            memory: Default::default(),
            module: None,
            runtime: Default::default(),
            binary: Default::default(),
            driver: None,
        }
    }
}

#[derive(Deserialize, Default, Debug)]
pub struct Memory {
    pub size: Option<String>,
    pub shared_size: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
pub struct Binary {
    pub path: String,
}

#[derive(Deserialize, Default, Debug)]
pub struct Runtime {
    pub path: String,
    pub mem_size: Option<String> 
}

#[derive(Deserialize, Debug)]
pub struct Module {
    pub path: String,
    pub args: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Driver {
    pub name: String,
    pub path: String,
}
