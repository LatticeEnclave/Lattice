use device::device::DeviceInfo;
use std::{env, fs};

fn main() {
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();

    let file_path = &args[1];
    let data: Vec<u8> = fs::read(file_path).unwrap();

    //let uart = find_uart(&data).unwrap();
    let device = DeviceInfo::new(data.as_ptr()).unwrap();
    let uart = device.get_uart().unwrap();
    println!("Uart: {:?}", uart);
    let stride = 0x1 << uart.reg_shift;
    println!("Stride: {}", stride);
    let ptr = uart.addr;
    println!("Base addr: {:?}", ptr);
    let reg1 = unsafe { ptr.add(1 * stride) };
    println!("Reg1 addr: {:?}", reg1);
}
