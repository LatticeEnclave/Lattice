use alloc::{alloc_pages, free_pages};
use clap::Parser;
use config::{Binary, Config};
use core::slice;
use htee_channel::{
    enclave::client::{create_lde, create_lue, launch_enclave, resume_enclave},
    h2e::create_lse,
    info::*,
    proxy::proxy_system_call,
};
use loader::Loader;
use page::Page;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    os::unix::fs::MetadataExt,
};

mod alloc;
mod config;
mod ctl;
mod loader;
mod page;

#[derive(Parser)]
struct Cli {
    /// Config file path.
    /// Default value is config.toml
    #[arg(short, long)]
    config: Option<String>,
    #[arg(short, long, default_value_t = false)]
    lde: bool,
    #[arg(short, long, default_value_t = false)]
    lse: bool,
    // #[arg(short, long, default_value_t = false)]
    // hugepage: bool,
    // #[arg(short, long, action = clap::ArgAction::SetTrue)]
    // service: bool,
    // /// Memory size.
    // /// The requested memory size must bigger than the total size of `bin` and `rt`
    // #[arg(short, long)]
    // mem: Option<String>,
    // /// runtime path
    // #[arg(short, long)]
    // rt: Option<String>,
    // elf: String,
    /// binary path, override the binary path in config file
    binary: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let path = cli.config.clone().unwrap_or("config.toml".into());
    if cli.lde {
        cli_create_lde(&path, false);
    } else if cli.lse {
        cli_create_lse(&path);
    } else {
        cli_create_lue(&cli, &path, false);
    }
}

fn cli_create_lse(path: &str) {
    let config = load_toml(&path);
    let rt = &config.runtime;
    println!("[Client] Load runtime: {}", rt.path);

    let file = File::open(&rt.path).unwrap();
    // one page for meta page
    let size = file.metadata().unwrap().size() + 0x1000;

    println!("rt size: {}", size);
    // let mem_size = arser_mem_size(&config.memory.size.unwrap_or("8k".to_owned()));;
    let mem_size = rt
        .mem_size
        .as_ref()
        .map(|size| parser_mem_size(size))
        .unwrap_or(size as usize);

    let pages = alloc_pages((mem_size + 0x1000 - 1) as usize & !(0x1000 - 1), false);

    let page_ptr = pages.as_ptr() as *mut Page;
    let page_num = pages.len();
    println!(
        "memory region: {:#x} - {:#x}",
        page_ptr as usize,
        page_ptr as usize + page_num * 0x1000
    );

    let mut loader = Loader::new(pages);

    let _ = loader.alloc_meta_page();

    let rt = loader
        .mmap(&config.runtime.path)
        .expect(&format!("{} load failed\n", config.runtime.path));

    let load_info = LseInfo {
        mem: MemInfo {
            start: loader.get_start(),
            page_num,
        },
        rt: RtInfo {
            ptr: rt.as_ptr(),
            size: rt.len(),
        },
    };

    println!("create enclave");
    let (rc, eidx) = create_lse(&load_info as *const _);
    println!("lse created");
    println!("eidx: {eidx:#x}");
    // never stop
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn cli_create_lue(cli: &Cli, path: &str, hugepage: bool) {
    let mut config = load_toml(&path);
    if let Some(binary) = &cli.binary {
        config.binary = Some(Binary {
            path: binary.clone(),
        });
    }

    let mem_size = parser_mem_size(&config.memory.size.unwrap_or("8k".to_owned()));
    let shared_size = parser_mem_size(&config.memory.shared_size.unwrap_or("8k".to_owned()));

    // alloc continue memory area
    let pages = alloc_pages(mem_size, hugepage);

    let page_ptr = pages.as_ptr() as *mut Page;
    let page_num = pages.len();
    println!(
        "memory region: {:#x} - {:#x}",
        page_ptr as usize,
        page_ptr as usize + page_num * 0x1000
    );

    // loading start
    let mut loader = Loader::new(pages);

    let _ = loader.alloc_meta_page();

    // we need to load runtime at the begin of the entire memory area
    let rt = loader
        .mmap(&config.runtime.path)
        .expect(&format!("{} load failed\n", config.runtime.path));

    let elf = loader.mmap(&config.binary.unwrap().path).unwrap();

    // let shared = loader.alloc(shared_size, 0x1000);
    let shared = loader.alloc_tail(shared_size);

    let unused = loader.get_remain_page();

    let load_info = LueInfo {
        mem: MemInfo {
            start: loader.get_start(),
            page_num,
        },
        rt: RtInfo {
            ptr: rt.as_ptr(),
            size: rt.len(),
        },
        bin: BinInfo {
            ptr: elf.as_ptr(),
            size: elf.len(),
        },
        // mods: new_m_infos,
        shared: SharedInfo {
            ptr: shared.as_ptr(),
            size: shared.len(),
        },
        unused: UnusedInfo {
            start: unused.as_ptr(),
            size: unused.len(),
        },
    };

    println!("create enclave");
    let (rc, eidx) = create_lue(&load_info as *const _);
    if rc != 0 {
        panic!("create lue failed");
    }
    println!("lue created");
    println!("eidx: {eidx:#x}");
    loop_waiting_for_enclave(eidx);
    // let pages = unsafe { slice::from_raw_parts_mut(page_ptr, page_num) };
    // free_pages(pages);
    println!("enclave finished");
}

fn cli_create_lde(path: &str, hugepage: bool) {
    let config = load_toml(&path);
    let driver = config.driver.unwrap();
    println!("[Client] Load driver: {}", driver.name);

    let mem_size = parser_mem_size(&config.memory.size.unwrap_or("8k".to_owned()));
    println!("[Client] Memory size required: {}", mem_size);

    // alloc continue memory area
    let pages = alloc_pages(mem_size, hugepage);

    let page_ptr = pages.as_ptr() as *mut Page;
    let page_num = pages.len();
    println!(
        "memory region: {:#x} - {:#x}",
        page_ptr as usize,
        page_ptr as usize + page_num * 0x1000
    );

    // loading start
    let mut loader = Loader::new(pages);

    let _ = loader.alloc_meta_page();

    // we need to load runtime at the begin of the entire memory area
    let rt = loader
        .mmap(&config.runtime.path)
        .expect(&format!("{} load failed\n", config.runtime.path));

    let elf = loader.mmap(&driver.path).unwrap();

    let unused = loader.get_remain_page();

    let load_info = LdeInfo {
        mem: MemInfo {
            start: loader.get_start(),
            page_num: loader.pages.len(),
        },
        rt: RtInfo {
            ptr: rt.as_ptr(),
            size: rt.len(),
        },
        bin: BinInfo {
            ptr: elf.as_ptr(),
            size: elf.len(),
        },
        driver: get_driver_info(&driver.name),
        unused: UnusedInfo {
            start: unused.as_ptr(),
            size: unused.len(),
        },
    };

    println!(
        "[Client] driver position: {:#x}, size: {:#x}",
        load_info.driver.ptr as usize, load_info.driver.size
    );

    println!("create enclave");
    let eidx = create_lde(&load_info as *const _);
    println!("lde created");
    println!("eidx: {eidx:#x}");
    // never stop
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn loop_waiting_for_enclave(eidx: usize) {
    let (mut rc, mut arg_addr) = launch_enclave(eidx);

    if rc != 0 {
        println!("[client]: launch enclave failed. Error code: {}", rc);
        return;
    }

    //
    loop {
        if arg_addr > 0x10 {
            unsafe {
                proxy_system_call(arg_addr);
            }
            // println!("[client]: proxy ecall returns, the eidx is: {}", eidx);
        }

        // println!("[client]: resume enclave");
        (rc, arg_addr) = resume_enclave(eidx);
        if rc != 0 {
            println!("[client]: resume enclave failed. Error code: {}", rc);
            return;
        }
        if arg_addr == 0 {
            return;
        }
    }
}

fn load_toml(path: &str) -> Config {
    let mut cfg_content = String::new();
    let mut file = File::open(path).unwrap();
    file.read_to_string(&mut cfg_content).unwrap();
    let cfg: Config = toml::from_str(&cfg_content).unwrap();

    cfg
}

fn get_driver_info(name: &str) -> DriverInfo {
    println!("get driver info for {name}");
    let line = filter_proc_modules(name).unwrap();
    println!("driver info from /proc/modules:");
    println!("{line}");
    let start_addr = line
        .split_ascii_whitespace()
        .nth(5)
        // .last()
        .map(|s| parser_hex_str(s))
        .unwrap();

    println!("Driver start address: {:#x}", start_addr);
    let size = line
        .split_ascii_whitespace()
        .nth(1)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap();

    println!("driver size: {}", size);

    let sections = get_sections(name);

    println!("sections: {sections:#x?}");

    if start_addr == 0 {
        println!("driver {name} not found");
        println!("you may need to run the program with root permission");
        panic!();
    }

    // 获取字节切片（确保是 UTF-8，符合 Rust 字符串特性 [[5]]）
    let bytes = name.as_bytes();
    let mut name = [0u8; 64];

    // 计算实际复制长度（防止越界）
    let len = bytes.len().min(64);

    // 复制到数组（自动截断超长部分）
    name[..len].copy_from_slice(&bytes[..len]);

    DriverInfo {
        ptr: start_addr as *const u8,
        size,
        sections,
        name,
    }
}

fn get_sections(name: &str) -> Sections {
    Sections {
        text: cat_sys_module(name, ".text").unwrap_or(0),
        text_unlikely: cat_sys_module(name, ".text.unlikely").unwrap_or(0),
    }
}

fn cat_sys_module(name: &str, section: &str) -> Result<usize, std::io::Error> {
    let target = format!("/sys/module/{name}/sections/{section}");
    let reader = BufReader::new(File::open(target)?);
    let line = reader.lines().next().unwrap()?;
    Ok(parser_hex_str(&line))
}

fn parser_hex_str(raw: &str) -> usize {
    let without_prefix = raw.trim_start_matches("0x");
    usize::from_str_radix(without_prefix, 16).unwrap()
}

fn filter_proc_modules(name: &str) -> Result<String, std::io::Error> {
    filter_modules_from_reader(name, BufReader::new(File::open("/proc/modules")?))
}

fn filter_modules_from_reader<R: BufRead>(name: &str, reader: R) -> Result<String, std::io::Error> {
    for line in reader.lines() {
        let line = line?;
        if let Some(first_word) = line.split_ascii_whitespace().next() {
            if first_word == name {
                return Ok(line);
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "module not found",
    ))
}

fn parser_mem_size(s: &str) -> usize {
    let len = s.len();
    if s.ends_with("k") {
        s[0..(len - 1)].parse::<usize>().unwrap() * 1024
    } else if s.ends_with("m") {
        s[0..(len - 1)].parse::<usize>().unwrap() * 1024 * 1024
    } else {
        panic!()
    }
}
