use htee_device::device::DeviceInfo;
use std::{env, fs};

fn main() {
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();

    let file_path = &args[1];
    let data: Vec<u8> = fs::read(file_path).unwrap();

    let mut device = DeviceInfo::new(data.as_ptr()).unwrap();
    let mem = device.get_mem_regions();
    let region = mem.into_iter().next().unwrap();
    println!("origin region: {region}");

    device.update_mem_region_size(region.start as usize, 0x100);
    let region = device.get_mem_regions().into_iter().next().unwrap();

    println!("new region: {region}");

    let reserved = device.get_mem_region_reserved().into_iter().next().unwrap();
    println!("origin reserved region: {reserved}");

    device.update_reserved_mem_region_size(region.start as usize, reserved.size + 0x10000);
    let reserved = device.get_mem_region_reserved().into_iter().next().unwrap();
    println!("new reserved region: {reserved}");
}
