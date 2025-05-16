use std::process::Command;
use std::{env, thread};

fn main() {
    // 获取命令行参数迭代器
    let args: Vec<String> = env::args().collect();

    // 检查是否提供了足够的参数
    if args.len() < 3 {
        eprintln!("Usage: {} \"command\" count", args[0]);
        return;
    }

    // 解析参数
    let command_input = &args[1]; // 第一个参数是命令（带双引号）
    let count_str = &args[2]; // 第二个参数是数量

    // 去除命令两边的引号（如果有的话）
    let command = command_input
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(command_input);

    // 将数量转换为整数
    let count: u32 = match count_str.parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("Error: Second argument must be a number.");
            return;
        }
    };

    // 打印解析结果
    println!("Command: {}", command);
    println!("Count: {}", count);

    // 使用空格分割命令和参数
    let parts: Vec<&str> = command.split_whitespace().collect();
    let parts = Box::leak(Box::new(parts));
    if parts.is_empty() {
        eprintln!("Error: Command is empty.");
        return;
    }

    let mut handles = vec![];

    for i in 0..count {
        let cmd = command.to_string();
        let handle = thread::spawn(move || {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .expect("Failed to execute command");

            // 打印子进程输出
            println!(
                "Process {} output:\n{}",
                i,
                String::from_utf8_lossy(&output.stdout)
            );
            output
        });

        handles.push(handle);
    }

    // 等待所有线程完成，并收集输出
    for handle in handles {
        let _output = handle.join().unwrap();
    }
}
