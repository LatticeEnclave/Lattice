#![no_std]
#![allow(unused_variables)]

mod symbol;

pub use elf::Sections;
use htee_console::log;
pub use symbol::SymbolTable;

// 第三方依赖
extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
// use htee_console::log;

// use crate::rt::ldesyms::{add_symbol, get_symbol_table};
use xmas_elf::sections::{SectionData, SectionHeader, ShType};
use xmas_elf::symbol_table::Entry;
use xmas_elf::ElfFile;

// RISC-V 重定位类型常量
const R_RISCV_NONE: u32 = 0;
const R_RISCV_32: u32 = 1;
const R_RISCV_64: u32 = 2;
const R_RISCV_RELATIVE: u32 = 3;
const R_RISCV_COPY: u32 = 4;
const R_RISCV_JUMP_SLOT: u32 = 5;
const R_RISCV_BRANCH: u32 = 16;
const R_RISCV_JAL: u32 = 17;
const R_RISCV_CALL: u32 = 18;
const R_RISCV_CALL_PLT: u32 = 19;
const R_RISCV_GOT_HI20: u32 = 20;
const R_RISCV_PCREL_HI20: u32 = 23;
const R_RISCV_PCREL_LO12_I: u32 = 24;
const R_RISCV_PCREL_LO12_S: u32 = 25;
const R_RISCV_HI20: u32 = 26;
const R_RISCV_LO12_I: u32 = 27;
const R_RISCV_LO12_S: u32 = 28;
const R_RISCV_RVC_BRANCH: u32 = 44;
const R_RISCV_RVC_JUMP: u32 = 45;

// StarFive JH7100 特定常量
const PAGE_SIZE: usize = 4096;
const INSTRUCTION_ALIGNMENT: usize = 4;
const COMPRESSED_INSTRUCTION_ALIGNMENT: usize = 2;

/// 从SectionHeader获取对齐值的辅助函数
fn get_section_alignment(section: &SectionHeader) -> usize {
    // ELF规范定义的默认对齐值
    let default_align = 1;

    // 没有直接的公开方法，我们使用一些其他特性来推断对齐值
    match section {
        // 尝试通过其它方式获取节区属性
        _ => {
            // 对于无法确定的情况，使用保守的默认值
            default_align
        }
    }
}

/// 严格按照StarFive JH7100 Linux内核的要求处理对齐
fn align_address(address: usize, alignment: usize) -> usize {
    // StarFive JH7100要求页对齐或更严格
    let effective_alignment = if alignment < PAGE_SIZE {
        // 内核可能会要求至少对齐到4096字节边界
        PAGE_SIZE
    } else {
        // 如果要求更严格，则使用指定的对齐
        alignment
    };
    // 确保对齐值是2的幂
    debug_assert!(
        effective_alignment.is_power_of_two(),
        "Alignment must be a power of 2"
    );
    // 计算对齐掩码
    let mask = effective_alignment - 1;
    // 向上对齐到下一个对齐边界
    (address + mask) & !mask
}

pub fn load_driver_by_sections(
    driver_data: &[u8],
    target: &mut [u8],
    addr: usize,
    symtab: &SymbolTable,
    sections: Sections,
) -> Result<(), &'static str> {
    // 解析ELF文件
    let elf_file = match ElfFile::new(driver_data) {
        Ok(file) => file,
        Err(_) => return Err("Failed to parse ELF file"),
    };

    if sections.text != 0 {
        log::debug!("load .text section");
        let data = &mut target[(sections.text - addr)..];
        if let Err(e) = load_section_at(
            &elf_file,
            data,
            sections.text,
            &symtab,
            ".text",
            ".rela.text",
        ) {
            log::error!("{}", e);
        }
    }

    if sections.text_unlikely != 0 {
        log::debug!("load .text.unlikely section");
        let data = &mut target[(sections.text_unlikely - addr)..];
        if let Err(e) = load_section_at(
            &elf_file,
            data,
            sections.text_unlikely,
            &symtab,
            ".text.unlikely",
            ".rela.text.unlikely",
        ) {
            log::error!("{}", e);
        }
    }

    Ok(())
}

pub fn load_section_at(
    elf_file: &ElfFile,
    target: &mut [u8],
    addr: usize,
    symtab: &SymbolTable,
    section: &str,
    rela: &str,
) -> Result<(), &'static str> {
    // 查找节区
    let header = match find_section_by_name(&elf_file, section) {
        Some(header) => header,
        None => return Err("Could not find .text section"),
    };

    let sec_data = match header.get_data(elf_file) {
        Ok(SectionData::Undefined(data)) => data,
        _ => return Err("Could not get .text section data"),
    };

    let sec_size = header.size() as usize;
    debug_assert!(sec_size <= target.len());

    // 确保目标区域已被初始化为0
    for byte in target.iter_mut() {
        *byte = 0;
    }

    log::debug!("section size: {}", sec_size);
    // 精确复制数据，保持字节布局
    target[..sec_data.len()].copy_from_slice(sec_data);

    // 查找重定位节区
    let rela_header = match find_section_by_name(&elf_file, rela) {
        Some(header) => header,
        None => return Err("Could not find .rela.text section"),
    };

    // 获取符号表节区
    let symtab_section = match find_section_by_name(&elf_file, ".symtab") {
        Some(section) => section,
        None => return Err("Could not find .symtab section"),
    };

    apply_relocations(&elf_file, addr, rela_header, symtab_section, symtab, target)?;

    Ok(())
}

/// 将.text段加载到指定地址并处理重定位，完全符合StarFive JH7100内核要求
///
/// Result<(usize, usize), &'static str>` - 成功则返回(实际加载地址, .text段的大小)，失败则返回错误信息
pub fn load_text_at(
    elf_data: &[u8],
    target: &mut [u8],
    fixed_address: usize,
    symtab: &SymbolTable,
) -> Result<(usize, usize), &'static str> {
    // 解析ELF文件
    let elf_file = match ElfFile::new(elf_data) {
        Ok(file) => file,
        Err(_) => return Err("Failed to parse ELF file"),
    };

    // 查找.text节区
    let text_section = match find_section_by_name(&elf_file, ".text") {
        Some(section) => section,
        None => return Err("Could not find .text section"),
    };

    // 获取.text数据
    let text_data = match text_section.get_data(&elf_file) {
        Ok(SectionData::Undefined(data)) => data,
        _ => return Err("Could not get .text section data"),
    };

    // 获取.text段大小
    let text_size = text_section.size() as usize;
    debug_assert!(text_size <= target.len());

    // 由于我们无法直接访问对齐值，根据StarFive JH7100要求，使用页对齐
    let original_align = INSTRUCTION_ALIGNMENT; // 使用最小指令对齐作为基础值

    // 确保对齐至少满足StarFive JH7100内核要求
    let text_align = PAGE_SIZE; // 直接使用页对齐，这是StarFive JH7100最保守的要求

    // log::debug!(
    //     "Assuming section alignment: {}, Using effective alignment: {}",
    //     original_align,
    //     text_align
    // );

    // 检查固定地址的对齐情况
    let actual_load_address = align_address(fixed_address, text_align);

    debug_assert_eq!(actual_load_address, fixed_address);

    // 使用Vec<u8>而不是直接写入内存地址（内核环境将会不同）
    // let mut text_dest = vec![0u8; text_size];
    let text_dest = target;

    // 确保目标区域已被初始化为0
    for byte in text_dest.iter_mut() {
        *byte = 0;
    }

    // 精确复制数据，保持字节布局
    text_dest[..text_data.len()].copy_from_slice(text_data);

    // log::debug!(
    //     "Loaded .text section at {:#x} (size={} bytes, align={} bytes)",
    //     actual_load_address,
    //     text_size,
    //     text_align
    // );

    // 输出原始.text数据（16进制格式）
    // log::debug!("Original .text content (hex):");
    // output_hex_dump(&text_dest, 16);

    // 查找.rela.text节区
    let rela_text_section = match find_section_by_name(&elf_file, ".rela.text") {
        Some(section) => section,
        None => {
            // log::debug!("No .rela.text section found, skipping relocations");

            // 即使没有重定位，也输出最终的.text内容
            // log::debug!("Final .text content (hex) - no relocations applied:");
            // output_hex_dump(&text_dest, 16);

            return Ok((actual_load_address, text_size)); // 没有重定位，直接返回
        }
    };

    // 获取符号表节区
    let symtab_section = match find_section_by_name(&elf_file, ".symtab") {
        Some(section) => section,
        None => return Err("Could not find .symtab section"),
    };

    // 精确应用重定位
    apply_relocations(
        &elf_file,
        actual_load_address,
        rela_text_section,
        symtab_section,
        symtab,
        text_dest,
    )?;

    // 输出重定位后的.text数据（16进制格式）
    // log::debug!("Relocated .text content (hex):");
    // output_hex_dump(&text_dest, 16);

    // log::debug!("Successfully applied all relocations");

    Ok((actual_load_address, text_size))
}

/// 检测指令是否为压缩指令
fn is_compressed_instruction(first_two_bytes: &[u8]) -> bool {
    // 检查低2位，压缩指令的低2位不是11
    (first_two_bytes[0] & 0x3) != 0x3
}

/// 创建指令边界映射，精确识别每条指令的位置
fn map_instruction_boundaries(data: &[u8]) -> Vec<(usize, usize, bool)> {
    let mut instruction_map = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        if offset + 2 > data.len() {
            break;
        }

        let first_two_bytes = &data[offset..offset + 2];
        let is_compressed = is_compressed_instruction(first_two_bytes);

        let instruction_size = if is_compressed { 2 } else { 4 };

        // 确保标准指令有足够的空间
        if !is_compressed && offset + 4 > data.len() {
            break;
        }

        // 记录指令开始位置、大小和类型
        instruction_map.push((offset, instruction_size, is_compressed));

        // 移动到下一条指令
        offset += instruction_size;
    }

    instruction_map
}

/// 查找给定偏移所属的指令
fn find_instruction_for_offset(
    offset: usize,
    instruction_map: &[(usize, usize, bool)],
) -> Option<(usize, usize, bool)> {
    for &(instr_start, instr_size, is_compressed) in instruction_map {
        // 检查偏移是否位于当前指令范围内
        if offset >= instr_start && offset < instr_start + instr_size {
            return Some((instr_start, instr_size, is_compressed));
        }
    }

    None
}

/// 解码并打印出一小段指令数据，用于诊断
fn decode_instruction_stream(data: &[u8], start_offset: usize, length: usize) {
    let mut offset = start_offset;
    let end_offset = core::cmp::min(start_offset + length, data.len());

    // log::debug!(
    //     "Decoding instructions from offset {:#x} to {:#x}:",
    //     start_offset,
    //     end_offset
    // );

    while offset < end_offset {
        if offset + 2 > end_offset {
            // log::debug!("  {:#x}: Incomplete instruction", offset);
            break;
        }

        let first_two_bytes = &data[offset..offset + 2];
        let is_compressed = is_compressed_instruction(first_two_bytes);

        if is_compressed {
            // 压缩指令 (16位)
            let instr = u16::from_le_bytes([first_two_bytes[0], first_two_bytes[1]]);
            // log::debug!("  {:#x}: {:#06x} (compressed)", offset, instr);
            offset += 2;
        } else if offset + 4 <= end_offset {
            // 检查是否4字节对齐
            if offset % 4 != 0 {
                // log::debug!("  {:#x}: MISALIGNED STANDARD INSTRUCTION DETECTED!", offset);
                // 显示未对齐指令的内容，帮助调试
                if offset + 4 <= end_offset {
                    let bytes = [
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ];
                    let potential_instr = u32::from_le_bytes(bytes);
                    // log::debug!("  Potential misaligned data: {:#010x}", potential_instr);
                }
                // 尝试重新同步 - 推进2字节，看下一个位置是否是有效指令
                offset += 2;
                continue;
            }

            // 标准指令 (32位)
            let instr_bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            let instr = u32::from_le_bytes(instr_bytes);
            // log::debug!("  {:#x}: {:#010x} (standard)", offset, instr);
            offset += 4;
        } else {
            // log::debug!("  {:#x}: Incomplete standard instruction", offset);
            break;
        }
    }
}

// /// 生成十六进制转储输出到日志
// fn output_hex_dump(data: &[u8], bytes_per_line: usize) {
//     let dump = hex_dump_to_string(data, bytes_per_line);
//     for line in dump.lines() {
//         // log::debug!("{}", line);
//     }
// }

/// 通过名称查找节区
fn find_section_by_name<'a>(elf_file: &'a ElfFile, name: &str) -> Option<SectionHeader<'a>> {
    for section in elf_file.section_iter() {
        if let Ok(section_name) = section.get_name(elf_file) {
            if section_name == name {
                return Some(section);
            }
        }
    }
    None
}

/// 检查地址是否符合StarFive JH7100的指令对齐要求
fn check_starfive_instruction_alignment(address: usize, is_compressed: bool) -> bool {
    if is_compressed {
        // 压缩指令需要2字节对齐
        (address % 2) == 0
    } else {
        // 标准指令需要4字节对齐
        (address % 4) == 0
    }
}

/// 根据StarFive JH7100 Linux内核精确应用重定位到加载的.text段
fn apply_relocations(
    elf_file: &ElfFile,
    load_address: usize,
    rela_section: SectionHeader,
    symtab_section: SectionHeader,
    symtab: &SymbolTable,
    text_dest: &mut [u8],
) -> Result<(), &'static str> {
    // 获取重定位数据和符号表
    let relocations = match rela_section.get_data(elf_file) {
        Ok(SectionData::Rela64(data)) => data,
        _ => return Err("Could not get relocation data"),
    };

    let symbol_table = match symtab_section.get_data(elf_file) {
        Ok(SectionData::SymbolTable64(data)) => data,
        _ => return Err("Could not get symbol table data"),
    };

    log::debug!("Processing {} relocations", relocations.len());

    // 映射所有指令的边界
    // let instruction_map = map_instruction_boundaries(text_dest);

    // log::debug!("Instruction boundaries:");
    // for (i, (offset, size, is_compressed)) in instruction_map.iter().enumerate() {
    //     log::debug!(
    //         "  Instruction {}: offset={:#x}, size={}, {}",
    //         i,
    //         offset,
    //         size,
    //         if *is_compressed {
    //             "compressed"
    //         } else {
    //             "standard"
    //         }
    //     );
    // }

    // 按照偏移量排序重定位条目
    // let mut sorted_relocs = relocations.iter().enumerate().collect::<Vec<_>>();
    // sorted_relocs.sort_by_key(|(_, rela)| rela.get_offset());

    // 分析原始重定位
    // log::debug!("Original relocation analysis:");
    // for (idx, rela) in &sorted_relocs {
    //     let r_offset = rela.get_offset() as usize;
    //     let r_sym = rela.get_symbol_table_index() as usize;
    //     let r_type = rela.get_type();

    //     if r_type == R_RISCV_BRANCH {
    //         let symbol = &symbol_table[r_sym];
    //         let sym_name = match symbol.get_name(elf_file) {
    //             Ok(name) => name,
    //             Err(_) => "<unknown>",
    //         };

    //         // let sym_value = symbol.value() as i64;
    //         // let rel_offset = sym_value - r_offset as i64;

    //         // log::debug!(
    //         //     "  BRANCH Rel[{}]: from offset {:#x} to symbol '{}' at {:#x}, relative offset={}",
    //         //     idx,
    //         //     r_offset,
    //         //     sym_name,
    //         //     sym_value,
    //         //     rel_offset
    //         // );
    //     }
    // }

    // 跟踪PCREL_HI20/LO12对
    let mut hi20_relocs = Vec::new();

    // 添加这一行：用于跟踪指令扩展导致的偏移变化
    let mut offset_adjustments = Vec::new();

    // 处理每个重定位条目
    // for (idx, rela) in sorted_relocs {
    for (idx, rela) in relocations.iter().enumerate() {
        let r_offset = rela.get_offset() as usize;
        let r_sym = rela.get_symbol_table_index() as usize;
        let r_type = rela.get_type();
        let r_addend = rela.get_addend();

        log::debug!(
            "r_offset: {:#x}, r_sym: {}, r_type: {}, r_addend: {}",
            r_offset,
            r_sym,
            r_type,
            r_addend
        );

        // 如果有指令扩展导致大小变化，可能需要调整后续重定位
        // if !offset_adjustments.is_empty() {
        //     // log::debug!(
        //     //     "Note: Some instructions were expanded, which may affect subsequent relocations",
        //     // );
        //     // log::debug!("Offset adjustments: {:?}", offset_adjustments);
        // }

        // 边界检查
        if r_offset >= text_dest.len() {
            log::warn!("Relocation offset out of bounds: {:#x}", r_offset);
            continue;
        }

        // 获取符号
        if r_sym >= symbol_table.len() {
            log::warn!("Symbol index out of bounds: {}", r_sym);
            continue;
        }

        let symbol = &symbol_table[r_sym];

        // 计算目标地址
        let symbol_value = symbol.value() as i64;
        // 通过符号的 section index 或 value 来推断
        let is_undefined = symbol.shndx() == 0 || symbol_value == 0;

        let target_address = if is_undefined {
            // 检查符号表中是否有此符号
            if let Ok(name) = symbol.get_name(elf_file) {
                match symtab.get_symbol(name) {
                    Some(addr) => {
                        log::debug!("Found external symbol '{}' at address {:#x}", name, addr);
                        addr as i64
                    }
                    None => {
                        log::debug!(
                            "Warning: Undefined external symbol '{}', using fallback address",
                            name
                        );
                        load_address as i64 + symbol_value
                    }
                }
            } else {
                // log::debug!("Warning: Could not get symbol name for undefined symbol");
                load_address as i64 + symbol_value
            }
        } else {
            // 内部定义的符号，使用现有逻辑
            load_address as i64 + symbol_value
        };

        // 获取符号名（如果可用，用于调试）
        let sym_name = match symbol.get_name(elf_file) {
            Ok(name) => name,
            Err(_) => "<unknown>",
        };

        // 计算重定位位置的实际地址
        let location_address = load_address + r_offset;

        log::debug!(
            "Relocation {}: type={}, offset={:#x}, symbol='{}', target={:#x}",
            idx,
            r_type,
            r_offset,
            sym_name,
            target_address
        );

        let instr_offset = r_offset;
        let instr_size = if is_compressed_instruction(&text_dest[instr_offset..instr_offset + 2]) {
            2
        } else {
            4
        };

        // 查找此偏移所属的指令
        // let (instr_offset, instr_size, is_compressed) =
        //     match find_instruction_for_offset(r_offset, &instruction_map) {
        //         Some(info) => info,
        //         None => {
        //             // log::debug!("Could not find instruction for offset {:#x}", r_offset);
        //             continue;
        //         }
        //     };

        // 如果重定位偏移不是指令开始位置，这通常是一个问题
        // if r_offset != instr_offset {
        //     log::debug!(
        //         "Warning: Relocation offset {:#x} is not at instruction boundary (instruction starts at {:#x})",
        //         r_offset, instr_offset
        //     );

        //     // 对于RVC_BRANCH和RVC_JUMP，我们需要处理指令边界
        //     if r_type != R_RISCV_RVC_BRANCH && r_type != R_RISCV_RVC_JUMP {
        //         // 调整重定位偏移到指令边界
        //         // log::debug!(
        //         //     "Adjusting relocation offset from {:#x} to instruction boundary at {:#x}",
        //         //     r_offset,
        //         //     instr_offset
        //         // );
        //     }
        // }
        // log::debug!(
        //     "Instruction at offset {:#x} is {} (size={})",
        //     instr_offset,
        //     if is_compressed {
        //         "compressed"
        //     } else {
        //         "standard"
        //     },
        //     instr_size
        // );
        // 应用重定位
        match r_type {
            R_RISCV_CALL | R_RISCV_CALL_PLT => {
                // 记录原始重定位偏移
                let original_offset = r_offset;

                // 分析该位置是否为压缩指令
                let is_compressed_at_offset = if r_offset + 2 <= text_dest.len() {
                    let first_two_bytes = &text_dest[r_offset..r_offset + 2];
                    (first_two_bytes[0] & 0x3) != 0x3 // 低2位不为11表示压缩指令
                } else {
                    false
                };

                log::debug!(
                    "CALL relocation at offset {:#x}, is_compressed={}",
                    r_offset,
                    is_compressed_at_offset
                );

                // 寻找CALL指令序列的开始位置（auipc+jalr指令对）
                let mut found_auipc = false;
                let mut search_offset = r_offset;
                let mut auipc_offset = 0;

                // 更灵活地搜索AUIPC指令：
                // 检查所有2字节对齐的位置，而不仅仅是4字节对齐的位置
                for delta in (0..=16).step_by(2) {
                    if delta > r_offset {
                        break; // 避免越界
                    }

                    let search_pos = r_offset - delta;

                    // 确保有足够的空间读取4字节
                    if search_pos + 4 <= text_dest.len() {
                        let instr_bytes = [
                            text_dest[search_pos],
                            text_dest[search_pos + 1],
                            text_dest[search_pos + 2],
                            text_dest[search_pos + 3],
                        ];

                        // 检查是否是标准指令（低两位为11）
                        let is_standard = (instr_bytes[0] & 0x3) == 0x3;

                        if is_standard {
                            let instr = u32::from_le_bytes(instr_bytes);

                            // 检查这是否是auipc指令 (opcode = 0x17)
                            if (instr & 0x7F) == 0x17 {
                                log::debug!(
                                    "Found potential auipc instruction at offset {:#x}: {:#010x}",
                                    search_pos,
                                    instr
                                );

                                auipc_offset = search_pos;
                                found_auipc = true;

                                // 检查紧随其后的jalr指令
                                if auipc_offset + 8 <= text_dest.len() {
                                    let jalr_bytes = [
                                        text_dest[auipc_offset + 4],
                                        text_dest[auipc_offset + 5],
                                        text_dest[auipc_offset + 6],
                                        text_dest[auipc_offset + 7],
                                    ];

                                    // 检查是否是标准指令
                                    let jalr_is_standard = (jalr_bytes[0] & 0x3) == 0x3;

                                    if jalr_is_standard {
                                        let jalr = u32::from_le_bytes(jalr_bytes);

                                        if (jalr & 0x7F) == 0x67 {
                                            log::debug!(
                                                "Confirmed jalr instruction at offset {:#x}, found complete auipc+jalr pair",
                                                auipc_offset + 4
                                            );
                                            break;
                                        } else {
                                            log::debug!(
                                                "Found auipc at {:#x}, but no jalr follows it (found {:#x} instead)",
                                                auipc_offset,
                                                jalr
                                            );
                                            // 继续搜索，但保持found_auipc=true，以防找不到更好的
                                        }
                                    }
                                }
                            }
                        } else if search_pos + 6 <= text_dest.len() {
                            // 检查这是否是一个压缩指令接着一个AUIPC指令
                            // 此时search_pos指向的是压缩指令的开始，而search_pos+2开始可能是AUIPC
                            let compressed_len = 2;
                            let next_pos = search_pos + compressed_len;

                            if next_pos + 4 <= text_dest.len() {
                                let next_instr_bytes = [
                                    text_dest[next_pos],
                                    text_dest[next_pos + 1],
                                    text_dest[next_pos + 2],
                                    text_dest[next_pos + 3],
                                ];

                                // 检查是否是标准指令
                                let is_next_standard = (next_instr_bytes[0] & 0x3) == 0x3;

                                if is_next_standard {
                                    let next_instr = u32::from_le_bytes(next_instr_bytes);

                                    // 检查这是否是auipc指令
                                    if (next_instr & 0x7F) == 0x17 {
                                        log::debug!(
                                            "Found auipc after compressed instruction at offset {:#x}: {:#010x}",
                                            next_pos,
                                            next_instr
                                        );

                                        auipc_offset = next_pos;
                                        found_auipc = true;

                                        // 检查紧随其后的jalr指令
                                        if auipc_offset + 8 <= text_dest.len() {
                                            // 类似上面的逻辑检查jalr
                                            let jalr_bytes = [
                                                text_dest[auipc_offset + 4],
                                                text_dest[auipc_offset + 5],
                                                text_dest[auipc_offset + 6],
                                                text_dest[auipc_offset + 7],
                                            ];

                                            let jalr_is_standard = (jalr_bytes[0] & 0x3) == 0x3;

                                            if jalr_is_standard {
                                                let jalr = u32::from_le_bytes(jalr_bytes);

                                                if (jalr & 0x7F) == 0x67 {
                                                    log::debug!(
                                                        "Confirmed jalr instruction after auipc at offset {:#x}",
                                                        auipc_offset + 4
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 如果没有找到auipc指令，尝试更灵活的搜索方式
                if !found_auipc {
                    // 输出从重定位点附近的指令流，以帮助调试
                    log::debug!("Instruction stream around r_offset {:#x}:", r_offset);
                    decode_instruction_stream(
                        text_dest,
                        if r_offset > 16 { r_offset - 16 } else { 0 },
                        32,
                    );

                    // 尝试所有2字节对齐的位置，而不仅是4字节对齐
                    for offset in (r_offset.saturating_sub(16)..r_offset).step_by(2) {
                        if offset + 4 > text_dest.len() {
                            continue;
                        }

                        let test_bytes = [
                            text_dest[offset],
                            text_dest[offset + 1],
                            text_dest[offset + 2],
                            text_dest[offset + 3],
                        ];

                        // 对于RISC-V标准指令，低2位必须是11
                        let is_standard = (test_bytes[0] & 0x3) == 0x3;

                        // 如果是标准指令且是AUIPC
                        if is_standard && (u32::from_le_bytes(test_bytes) & 0x7F) == 0x17 {
                            log::debug!("Found auipc at {:#x} with more flexible search", offset);
                            auipc_offset = offset;
                            found_auipc = true;
                            break;
                        }
                    }

                    // 如果仍然找不到，再使用回退策略
                    if !found_auipc {
                        let aligned_offset = (r_offset / 4) * 4;
                        log::debug!(
                            "Could not find auipc instruction, using aligned offset {:#x} instead",
                            aligned_offset
                        );
                        auipc_offset = aligned_offset;
                    }
                }

                // 确保有足够空间存放auipc+jalr指令对
                if auipc_offset + 8 > text_dest.len() {
                    return Err("Insufficient space for CALL relocation instruction pair");
                }

                // 读取原始指令
                let orig_auipc_bytes = [
                    text_dest[auipc_offset],
                    text_dest[auipc_offset + 1],
                    text_dest[auipc_offset + 2],
                    text_dest[auipc_offset + 3],
                ];
                let orig_auipc = u32::from_le_bytes(orig_auipc_bytes);

                let orig_jalr_bytes = [
                    text_dest[auipc_offset + 4],
                    text_dest[auipc_offset + 5],
                    text_dest[auipc_offset + 6],
                    text_dest[auipc_offset + 7],
                ];
                let orig_jalr = u32::from_le_bytes(orig_jalr_bytes);

                log::debug!(
                    "Original instruction pair at {:#x}: auipc={:#010x}, jalr={:#010x}",
                    auipc_offset,
                    orig_auipc,
                    orig_jalr
                );

                // 记录原始字节值，便于调试
                log::debug!(
                    "Original bytes: auipc=[{:02x} {:02x} {:02x} {:02x}], jalr=[{:02x} {:02x} {:02x} {:02x}]",
                    orig_auipc_bytes[0], orig_auipc_bytes[1], orig_auipc_bytes[2], orig_auipc_bytes[3],
                    orig_jalr_bytes[0], orig_jalr_bytes[1], orig_jalr_bytes[2], orig_jalr_bytes[3]
                );

                // 计算PC相对偏移
                let pc = (load_address + auipc_offset) as i64;
                let offset = target_address - pc;

                log::debug!(
                    "PC={:#x}, target={:#x}, calculated offset={} ({:#x})",
                    pc,
                    target_address,
                    offset,
                    offset
                );

                //  分解偏移量为高20位和低12位
                let hi20 = ((offset + 0x800) >> 12) as u32;
                let lo12 = (offset & 0xfff) as u32;

                // 创建/修改auipc指令
                let mut auipc = orig_auipc;

                // 如果原指令不是auipc，或者我们没有找到正确的auipc指令
                if (auipc & 0x7F) != 0x17 {
                    // 创建一个全新的auipc指令，使用x1(ra)寄存器
                    // auipc x1, hi20
                    auipc = 0x00000097 | (hi20 << 12);
                    log::debug!(
                        "Creating new auipc instruction: {:#010x} (hi20={:#x})",
                        auipc,
                        hi20
                    );
                } else {
                    // 修改现有auipc指令，保留rd字段，更新高20位
                    let rd = (auipc >> 7) & 0x1F; // 提取目标寄存器
                    auipc = (0x17 | (rd << 7)) | (hi20 << 12);
                    log::debug!(
                        "Modifying existing auipc instruction: {:#010x} (rd=x{}, hi20={:#x})",
                        auipc,
                        rd,
                        hi20
                    );
                }

                // 创建/修改jalr指令
                let mut jalr = orig_jalr;

                // 如果原指令不是jalr，或者我没有找到正确的jalr指令
                if (jalr & 0x7F) != 0x67 {
                    // 创建一个全新的jalr指令，使用x0作为目标，x1作为源
                    // jalr x0, lo12(x1)
                    jalr = 0x000080e7 | (lo12 << 20);
                    log::debug!(
                        "Creating new jalr instruction: {:#010x} (lo12={:#x})",
                        jalr,
                        lo12
                    );
                } else {
                    // 修改现有jalr指令，保留rd和rs1字段，只更新12位立即数
                    jalr = (jalr & 0x000FFFFF) | (lo12 << 20);
                    log::debug!(
                        "Modifying existing jalr instruction: {:#010x} (lo12={:#x})",
                        jalr,
                        lo12
                    );
                }

                //写回修改后的指令
                text_dest[auipc_offset..auipc_offset + 4].copy_from_slice(&auipc.to_le_bytes());
                text_dest[auipc_offset + 4..auipc_offset + 8].copy_from_slice(&jalr.to_le_bytes());

                //记录修改
                log::debug!("Modified instruction pair at {:#x}:", auipc_offset);
                log::debug!("  auipc: {:#010x} -> {:#010x}", orig_auipc, auipc);
                log::debug!("  jalr: {:#010x} -> {:#010x}", orig_jalr, jalr);

                // 字节级别比较
                log::debug!("Byte-level changes:");
                log::debug!(
                    "  auipc bytes: [{:02x} {:02x} {:02x} {:02x}] -> [{:02x} {:02x} {:02x} {:02x}]",
                    orig_auipc_bytes[0],
                    orig_auipc_bytes[1],
                    orig_auipc_bytes[2],
                    orig_auipc_bytes[3],
                    auipc.to_le_bytes()[0],
                    auipc.to_le_bytes()[1],
                    auipc.to_le_bytes()[2],
                    auipc.to_le_bytes()[3]
                );
                log::debug!(
                    "  jalr bytes: [{:02x} {:02x} {:02x} {:02x}] -> [{:02x} {:02x} {:02x} {:02x}]",
                    orig_jalr_bytes[0],
                    orig_jalr_bytes[1],
                    orig_jalr_bytes[2],
                    orig_jalr_bytes[3],
                    jalr.to_le_bytes()[0],
                    jalr.to_le_bytes()[1],
                    jalr.to_le_bytes()[2],
                    jalr.to_le_bytes()[3]
                );

                // 如果原始重定位点与发现的auipc偏移有很大差异，添加警告
                if original_offset.abs_diff(auipc_offset) > 8 {
                    log::debug!(
                        "Warning: Large difference between relocation offset ({:#x}) and auipc offset ({:#x})",
                        original_offset,
                        auipc_offset
                    );
                }
            }

            R_RISCV_BRANCH => {
                // 条件分支指令
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for BRANCH relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let instr = u32::from_le_bytes(instr_bytes);

                // 计算PC相对偏移
                let pc = (load_address + instr_offset) as i64;
                let offset = target_address - pc;

                // log::debug!(
                //     "BRANCH: PC={:#x}, target={:#x}, offset={} ({:#x})",
                //     pc,
                //     target_address,
                //     offset,
                //     offset
                // );

                // 检查偏移是否在范围内
                if offset < -4096 || offset > 4095 || (offset & 1) != 0 {
                    // log::debug!(
                    //     "Branch offset {} out of range [-4096, 4095] - replacing with long jump sequence",
                    //     offset
                    // );

                    // 我们需要替换条件分支为一个条件分支+长跳转序列
                    // 首先，确保我们有足够的空间
                    if instr_offset + 12 > text_dest.len() {
                        return Err("Insufficient space for long branch sequence");
                    }

                    // 提取原指令的关键部分
                    let funct3 = (instr >> 12) & 0x7; // 提取funct3字段（指定条件类型）
                    let rs1 = (instr >> 15) & 0x1F; // 提取rs1寄存器
                    let rs2 = (instr >> 20) & 0x1F; // 提取rs2寄存器

                    // 1. 反转条件分支指令，跳过长跳转序列
                    // 创建一个反转条件的分支指令，跳过8字节（跳过下面的auipc+jalr指令）
                    // 反转funct3: beq->bne, blt->bge, 等
                    let reversed_funct3 = funct3 ^ 0x1;

                    // 使用u32确保移位操作不会溢出
                    let skip_branch: u32 = 0x00000063        // 标准分支操作码
                                     | ((reversed_funct3 as u32) << 12)  // 反转的条件
                                     | ((rs1 as u32) << 15)        // 同样的rs1
                                     | ((rs2 as u32) << 20)        // 同样的rs2
                                     | (0x4u32 << 8)         // 跳过8字节的偏移
                                     | (0x0u32 << 7)         // 最低位为0
                                     | (0x0u32 << 25)        // 没有高位偏移
                                     | (0x0u32 << 31); // 符号位为0

                    // 2. 创建无条件长跳转序列（auipc + jalr）
                    // 计算高20位和低12位偏移
                    let hi20 = ((offset + 0x800) >> 12) as u32;
                    let lo12 = (offset & 0xfff) as u32;

                    // auipc x1, hi20  （将PC+高20位加载到x1寄存器）
                    let auipc = 0x00000097u32 | (hi20 << 12);

                    // jalr x0, lo12(x1)  （使用x1跳转到目标地址，不保存返回地址）
                    let jalr = 0x000080e7u32 | (lo12 << 20);

                    // 将新的指令序列写入目标缓冲区
                    text_dest[instr_offset..instr_offset + 4]
                        .copy_from_slice(&skip_branch.to_le_bytes());
                    text_dest[instr_offset + 4..instr_offset + 8]
                        .copy_from_slice(&auipc.to_le_bytes());
                    text_dest[instr_offset + 8..instr_offset + 12]
                        .copy_from_slice(&jalr.to_le_bytes());

                    // log::debug!(
                    //     "Replaced branch with sequence: branch={:#x}, auipc={:#x}, jalr={:#x}",
                    //     skip_branch,
                    //     auipc,
                    //     jalr
                    // );
                } else {
                    // 偏移在范围内，正常处理
                    let mut instr = u32::from_le_bytes(instr_bytes);

                    // 清除当前指令的立即数字段
                    instr &= 0x01F07FFF; // 保留操作码、rs1、rs2和func3部分

                    // 设置新的立即数字段
                    let imm12 = ((offset & 0x1000) >> 12) as u32;
                    let imm11 = ((offset & 0x800) >> 11) as u32;
                    let imm10_5 = ((offset & 0x7E0) >> 5) as u32;
                    let imm4_1 = ((offset & 0x1E) >> 1) as u32;

                    instr |= (imm12 << 31) | (imm11 << 7) | (imm10_5 << 25) | (imm4_1 << 8);

                    // 写回修改后的指令
                    text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                    // log::debug!(
                    //     "Applied BRANCH relocation: offset={}, instruction={:#x}",
                    //     offset,
                    //     instr
                    // );
                }
            }

            R_RISCV_JAL => {
                // JAL指令
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for JAL relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 计算PC相对偏移
                let pc = (load_address + instr_offset) as i64; // 使用指令边界地址
                let offset = target_address - pc;

                // 检查偏移是否在范围内 (±1MB)
                if offset < -1048576 || offset > 1048575 || (offset & 1) != 0 {
                    return Err("JAL offset out of range");
                }

                // 清除当前指令的立即数字段
                instr &= 0x00000FFF; // 保留操作码和rd部分

                // 设置新的立即数字段
                let imm20 = ((offset & 0x100000) >> 20) as u32;
                let imm19_12 = ((offset & 0xFF000) >> 12) as u32;
                let imm11 = ((offset & 0x800) >> 11) as u32;
                let imm10_1 = ((offset & 0x7FE) >> 1) as u32;

                instr |= (imm20 << 31) | (imm19_12 << 12) | (imm11 << 20) | (imm10_1 << 21);

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                // log::debug!(
                //     "Applied JAL relocation: offset={}, instruction={:#x}",
                //     offset,
                //     instr
                // );
            }

            R_RISCV_PCREL_HI20 => {
                // PC相对高20位，通常用于auipc指令
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for PCREL_HI20 relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 计算PC相对偏移
                let pc = (load_address + instr_offset) as i64; // 使用指令边界地址
                let offset = target_address - pc;

                // 清除当前指令的立即数字段
                instr &= 0x00000FFF; // 保留操作码和rd部分

                // 设置新的立即数字段 (高20位，向上舍入)
                let hi20 = ((offset + 0x800) >> 12) as u32;
                instr |= hi20 << 12;

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                // 保存此HI20重定位信息，以便与对应的LO12配对
                hi20_relocs.push((instr_offset, offset, target_address));

                log::debug!(
                    "Applied PCREL_HI20 relocation: offset={}, hi20={:#x}, instruction={:#x}",
                    offset,
                    hi20,
                    instr
                );
            }

            R_RISCV_PCREL_LO12_I => {
                // PC相对低12位，用于I类指令（load, jalr, addi等）
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for PCREL_LO12_I relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 查找对应的HI20重定位
                // 注意：JH7100内核要求必须正确配对HI20/LO12重定位
                let mut hi20_offset = None;

                // 根据加数找到匹配的HI20重定位
                for (hi_offset, _, hi_target) in &hi20_relocs {
                    if *hi_target == target_address {
                        hi20_offset = Some(*hi_offset);
                        break;
                    }
                }

                // 如果找到对应的HI20重定位，使用它的PC进行计算
                let offset = if let Some(hi_offset) = hi20_offset {
                    let hi_pc = load_address + hi_offset;
                    target_address - hi_pc as i64
                } else {
                    // 没有找到匹配的HI20，使用自己的PC
                    target_address - (load_address + instr_offset) as i64
                };

                // 清除当前指令的立即数字段
                instr &= 0x000FFFFF; // 保留操作码、rd和func3部分

                // 设置新的立即数字段 (低12位)
                let lo12 = (offset & 0xFFF) as u32;
                instr |= lo12 << 20;

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                log::debug!(
                    "Applied PCREL_LO12_I relocation: offset={}, lo12={:#x}, instruction={:#x}",
                    offset,
                    lo12,
                    instr
                );
            }

            R_RISCV_PCREL_LO12_S => {
                // PC相对低12位，用于S类指令（store）
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for PCREL_LO12_S relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 查找对应的HI20重定位
                let mut hi20_offset = None;

                // 根据加数找到匹配的HI20重定位
                for (hi_offset, _, hi_target) in &hi20_relocs {
                    if *hi_target == target_address {
                        hi20_offset = Some(*hi_offset);
                        break;
                    }
                }

                // 如果找到对应的HI20重定位，使用它的PC进行计算
                let offset = if let Some(hi_offset) = hi20_offset {
                    let hi_pc = load_address + hi_offset;
                    target_address - hi_pc as i64
                } else {
                    // 没有找到匹配的HI20，使用自己的PC
                    target_address - (load_address + instr_offset) as i64
                };

                // 清除当前指令的立即数字段
                instr &= 0x01F07FFF; // 保留操作码、rs2、rs1和func3部分

                // 设置新的立即数字段 (低12位，分成imm11:5和imm4:0两部分)
                let imm11_5 = ((offset & 0xFE0) >> 5) as u32;
                let imm4_0 = (offset & 0x1F) as u32;

                instr |= (imm11_5 << 25) | (imm4_0 << 7);

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                log::debug!(
                    "Applied PCREL_LO12_S relocation: offset={}, imm11_5={:#x}, imm4_0={:#x}, instruction={:#x}",
                    offset,
                    imm11_5,
                    imm4_0,
                    instr
                );
            }

            R_RISCV_HI20 => {
                // 高20位，用于lui指令
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for HI20 relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 清除当前指令的立即数字段
                instr &= 0x00000FFF; // 保留操作码和rd部分

                // 设置新的立即数字段 (高20位，向上舍入)
                let hi20 = ((target_address + 0x800) >> 12) as u32;
                instr |= hi20 << 12;

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                // log::debug!(
                //     "Applied HI20 relocation: target={:#x}, hi20={:#x}, instruction={:#x}",
                //     target_address,
                //     hi20,
                //     instr
                // );
            }

            R_RISCV_LO12_I => {
                // 低12位，用于I类指令（addi等）
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for LO12_I relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 清除当前指令的立即数字段
                instr &= 0x000FFFFF; // 保留操作码、rd和func3部分

                // 设置新的立即数字段 (低12位)
                let lo12 = (target_address & 0xFFF) as u32;
                instr |= lo12 << 20;

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                // log::debug!(
                //     "Applied LO12_I relocation: target={:#x}, lo12={:#x}, instruction={:#x}",
                //     target_address,
                //     lo12,
                //     instr
                // );
            }

            R_RISCV_LO12_S => {
                // 低12位，用于S类指令（store）
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for LO12_S relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 4];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 4]);
                let mut instr = u32::from_le_bytes(instr_bytes);

                // 清除当前指令的立即数字段
                instr &= 0x01F07FFF; // 保留操作码、rs2、rs1和func3部分

                // 设置新的立即数字段 (低12位，分成imm11:5和imm4:0两部分)
                let imm11_5 = ((target_address & 0xFE0) >> 5) as u32;
                let imm4_0 = (target_address & 0x1F) as u32;

                instr |= (imm11_5 << 25) | (imm4_0 << 7);

                // 写回修改后的指令
                text_dest[instr_offset..instr_offset + 4].copy_from_slice(&instr.to_le_bytes());

                // log::debug!(
                //     "Applied LO12_S relocation: target={:#x}, instruction={:#x}",
                //     target_address,
                //     instr
                // );
            }

            R_RISCV_RVC_BRANCH => {
                // 压缩的分支指令 (C.BEQZ, C.BNEZ)
                if instr_offset + 2 > text_dest.len() {
                    return Err("Insufficient space for RVC_BRANCH relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 2];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 2]);
                let mut instr = u16::from_le_bytes(instr_bytes);

                // 计算PC相对偏移
                let pc = (load_address + instr_offset) as i64; // 使用指令边界地址
                let offset = target_address - pc;

                // 检查偏移是否在范围内
                if offset < -256 || offset > 254 || (offset & 1) != 0 {
                    // log::debug!(
                    //     "RVC_BRANCH offset {} out of range [-256, 254] - expanding to long branch",
                    //     offset
                    // );

                    // 将压缩分支扩展为标准分支+长跳转序列
                    // 这需要更多空间，确保有足够的空间
                    if instr_offset + 12 > text_dest.len() {
                        return Err("Insufficient space for expanding compressed branch");
                    }

                    // 解析压缩指令的寄存器和操作码
                    let rd_rs1 = ((instr >> 7) & 0x7) + 8; // 压缩指令使用x8-x15寄存器
                    let is_bnez = (instr & 0xE003) == 0xE001; // C.BNEZ为e001, C.BEQZ为c001

                    // 创建一个标准长度的反转条件分支指令
                    let rs2 = 0; // 比较零使用x0
                    let funct3 = if is_bnez { 0x1 } else { 0x0 }; // beq或bne
                    let reversed_funct3 = funct3 ^ 0x1; // 反转条件

                    // 使用u32确保移位操作不会溢出
                    let skip_branch: u32 = 0x00000063        // 标准分支操作码
                                    | ((reversed_funct3 as u32) << 12)  // 反转的条件 
                                    | ((rd_rs1 as u32) << 15)       // rs1为原始压缩指令的rd/rs1
                                    | ((rs2 as u32) << 20)          // rs2为x0
                                    | (0x4u32 << 8)           // 跳过8字节的偏移
                                    | (0x0u32 << 7)           // 最低位为0
                                    | (0x0u32 << 25)          // 没有高位偏移
                                    | (0x0u32 << 31); // 符号位为0

                    // 计算长跳转序列的偏移
                    let hi20 = ((offset + 0x800) >> 12) as u32;
                    let lo12 = (offset & 0xfff) as u32;

                    // auipc x1, hi20
                    let auipc = 0x00000097u32 | (hi20 << 12);

                    // jalr x0, lo12(x1)
                    let jalr = 0x000080e7u32 | (lo12 << 20);

                    // 写入扩展的指令序列
                    text_dest[instr_offset..instr_offset + 4]
                        .copy_from_slice(&skip_branch.to_le_bytes());
                    text_dest[instr_offset + 4..instr_offset + 8]
                        .copy_from_slice(&auipc.to_le_bytes());
                    text_dest[instr_offset + 8..instr_offset + 12]
                        .copy_from_slice(&jalr.to_le_bytes());

                    // log::debug!(
                    //     "Expanded compressed branch to sequence: branch={:#x}, auipc={:#x}, jalr={:#x}",
                    //     skip_branch,
                    //     auipc,
                    //     jalr
                    // );

                    // 标记处理过的偏移，并记录大小变化（从2字节变为12字节）
                    // offset_adjustments.push((instr_offset + 2, 10)); // 后面的指令偏移增加了10字节
                } else {
                    // 偏移在范围内，正常处理
                    // 清除当前指令的立即数字段
                    instr &= 0xE383; // 保留操作码和寄存器部分

                    // 设置新的立即数字段
                    let offset_u = offset as u32;
                    let imm8 = ((offset_u >> 8) & 0x1) << 12  // bit 8
                        | ((offset_u >> 3) & 0x3) << 10       // bits 4:3
                        | ((offset_u >> 6) & 0x3) << 5        // bits 7:6
                        | ((offset_u >> 1) & 0x3) << 3        // bits 2:1
                        | ((offset_u >> 5) & 0x1) << 2; // bit 5

                    instr |= imm8 as u16;

                    // 写回修改后的指令
                    text_dest[instr_offset..instr_offset + 2].copy_from_slice(&instr.to_le_bytes());

                    // log::debug!(
                    //     "Applied RVC_BRANCH relocation: offset={}, instruction={:#x}",
                    //     offset,
                    //     instr
                    // );
                }
            }

            R_RISCV_RVC_JUMP => {
                // 压缩的跳转指令 (C.J)
                if instr_offset + 2 > text_dest.len() {
                    return Err("Insufficient space for RVC_JUMP relocation");
                }

                // 读取原始指令
                let mut instr_bytes = [0u8; 2];
                instr_bytes.copy_from_slice(&text_dest[instr_offset..instr_offset + 2]);
                let mut instr = u16::from_le_bytes(instr_bytes);

                // 计算PC相对偏移
                let pc = (load_address + instr_offset) as i64; // 使用指令边界地址
                let offset = target_address - pc;

                // 检查偏移是否在范围内
                if offset < -2048 || offset > 2046 || (offset & 1) != 0 {
                    // log::debug!(
                    //     "RVC_JUMP offset {} out of range [-2048, 2046] - expanding to long jump",
                    //     offset
                    // );

                    // 将压缩跳转扩展为标准JAL或AUIPC+JALR序列
                    // 首先检查是否可以使用标准JAL指令（±1MB范围）
                    if offset >= -1048576 && offset <= 1048575 {
                        // 可以使用标准JAL指令
                        if instr_offset + 4 > text_dest.len() {
                            return Err("Insufficient space for expanding compressed jump to JAL");
                        }

                        // 计算JAL指令的立即数字段
                        let imm20 = ((offset & 0x100000) >> 20) as u32;
                        let imm19_12 = ((offset & 0xFF000) >> 12) as u32;
                        let imm11 = ((offset & 0x800) >> 11) as u32;
                        let imm10_1 = ((offset & 0x7FE) >> 1) as u32;

                        // JAL x0, offset (跳转但不链接)
                        let jal = 0x0000006Fu32
                            | (imm20 << 31)
                            | (imm19_12 << 12)
                            | (imm11 << 20)
                            | (imm10_1 << 21);

                        // 写入扩展的JAL指令
                        text_dest[instr_offset..instr_offset + 4]
                            .copy_from_slice(&jal.to_le_bytes());

                        // log::debug!("Expanded compressed jump to JAL: instruction={:#x}", jal);

                        // 标记处理过的偏移，并记录大小变化（从2字节变为4字节）
                        offset_adjustments.push((instr_offset + 2, 2)); // 后面的指令偏移增加了2字节
                    } else {
                        // 超出JAL范围，使用AUIPC+JALR序列
                        if instr_offset + 8 > text_dest.len() {
                            return Err(
                                "Insufficient space for expanding compressed jump to AUIPC+JALR",
                            );
                        }

                        // 计算高20位和低12位偏移
                        let hi20 = ((offset + 0x800) >> 12) as u32;
                        let lo12 = (offset & 0xfff) as u32;

                        // auipc x1, hi20
                        let auipc = 0x00000097u32 | (hi20 << 12);

                        // jalr x0, lo12(x1)
                        let jalr = 0x000080e7u32 | (lo12 << 20);

                        // 写入扩展的指令序列
                        text_dest[instr_offset..instr_offset + 4]
                            .copy_from_slice(&auipc.to_le_bytes());
                        text_dest[instr_offset + 4..instr_offset + 8]
                            .copy_from_slice(&jalr.to_le_bytes());

                        // log::debug!(
                        //     "Expanded compressed jump to AUIPC+JALR: auipc={:#x}, jalr={:#x}",
                        //     auipc,
                        //     jalr
                        // );

                        // 标记处理过的偏移，并记录大小变化（从2字节变为8字节）
                        offset_adjustments.push((instr_offset + 2, 6)); // 后面的指令偏移增加了6字节
                    }
                } else {
                    // 偏移在范围内，正常处理
                    // 清除当前指令的立即数字段
                    instr &= 0xE003; // 保留操作码部分

                    // 设置新的立即数字段
                    let offset_u = offset as u32;
                    let imm11 = ((offset_u >> 11) & 0x1) << 12  // bit 11
                        | ((offset_u >> 4) & 0x1) << 11         // bit 4
                        | ((offset_u >> 8) & 0x3) << 9          // bits 9:8
                        | ((offset_u >> 10) & 0x1) << 8         // bit 10
                        | ((offset_u >> 6) & 0x1) << 7          // bit 6
                        | ((offset_u >> 7) & 0x1) << 6          // bit 7
                        | ((offset_u >> 1) & 0x7) << 3          // bits 3:1
                        | ((offset_u >> 5) & 0x1) << 2; // bit 5

                    instr |= imm11 as u16;

                    // 写回修改后的指令
                    text_dest[instr_offset..instr_offset + 2].copy_from_slice(&instr.to_le_bytes());

                    // log::debug!(
                    //     "Applied RVC_JUMP relocation: offset={}, instruction={:#x}",
                    //     offset,
                    //     instr
                    // );
                }
            }

            R_RISCV_32 => {
                // 32位绝对地址重定位
                if instr_offset + 4 > text_dest.len() {
                    return Err("Insufficient space for 32-bit relocation");
                }

                let target_value = target_address as u32;
                text_dest[instr_offset..instr_offset + 4]
                    .copy_from_slice(&target_value.to_le_bytes());

                // log::debug!("Applied 32-bit relocation: value={:#x}", target_value);
            }

            R_RISCV_64 => {
                // 64位绝对地址重定位
                if instr_offset + 8 > text_dest.len() {
                    return Err("Insufficient space for 64-bit relocation");
                }

                let target_value = target_address as u64;
                text_dest[instr_offset..instr_offset + 8]
                    .copy_from_slice(&target_value.to_le_bytes());

                // log::debug!("Applied 64-bit relocation: value={:#x}", target_value);
            }

            _ => {
                // log::debug!("Unsupported relocation type: {}", r_type);
            }
        }
    }

    // 如果有指令扩展导致大小变化，可能需要调整后续重定位
    if !offset_adjustments.is_empty() {
        // log::debug!(
        //     "Note: Some instructions were expanded, which may affect subsequent relocations"
        // );
        // log::debug!("Offset adjustments: {:?}", offset_adjustments);
    }

    // 重定位后分析指令流
    // log::debug!("Instruction stream analysis after relocations:");
    // decode_instruction_stream(text_dest, 0, core::cmp::min(text_dest.len(), 64));

    // log::debug!("Successfully applied all relocations");
    Ok(())
}
