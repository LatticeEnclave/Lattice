use device::device::DeviceInfo;
use std::{env, fs};

fn main() {
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();

    let file_path = &args[1];
    let data: Vec<u8> = fs::read(file_path).unwrap();

    let device = DeviceInfo::new(data.as_ptr()).unwrap();
    let mem = device.get_mem_regions();
    println!("memory regions:");
    for region in mem {
        println!("{region}");
    }

    let reserved = device.get_mem_region_reserved();
    println!("reserved memory regions:");
    for region in reserved {
        println!("{region}");
    }

    //let node = device.fdt.find_node("/reserved-memory").unwrap();
    //let mut childs = node.children();
    //let c = childs.next().unwrap();
    //let reg = c.reg().unwrap();
    //for mem in reg {
    //    println!("{mem:#x?}");
    //}
}
