use htee_device::dma;
use sbi::TrapRegs;

use crate::{Error, sm};

/// Instruction types for memory operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemInstructionType {
    /// Store instruction (writing to memory)
    RegStore {
        /// Register containing the data to write
        data_reg: u8,
        /// Size of the store operation in bytes
        size: u8,
    },
    /// Not a memory write instruction
    NotStore,
    /// Invalid or unrecognized instruction
    Invalid,
}

/// Analyzes an instruction at the given address to determine if it's a memory write instruction
///
/// # Arguments
/// * `inst` - Instruction to analyze
///
/// # Returns
/// * `MemInstructionType` - Information about the instruction if it's a store instruction
pub fn analyze_instruction(inst: u32) -> MemInstructionType {
    // RISC-V instruction format for stores:
    // SW:  imm[11:5] rs2 rs1 010 imm[4:0] 0100011
    // SH:  imm[11:5] rs2 rs1 001 imm[4:0] 0100011
    // SB:  imm[11:5] rs2 rs1 000 imm[4:0] 0100011
    // SD:  imm[11:5] rs2 rs1 011 imm[4:0] 0100011

    // Extract opcode (lowest 7 bits)
    let opcode = inst & 0x7f;

    if opcode == 0x23 {
        // Extract funct3 field (bits 12-14) to determine store type
        let funct3 = (inst >> 12) & 0x7;

        // Extract rs2 (data register, bits 20-24)
        let rs2 = ((inst >> 20) & 0x1f) as u8;

        // Determine size based on funct3
        let size = match funct3 {
            0 => 1, // SB - byte
            1 => 2, // SH - halfword
            2 => 4, // SW - word
            3 => 8, // SD - doubleword
            _ => return MemInstructionType::Invalid,
        };

        return MemInstructionType::RegStore {
            data_reg: rs2,
            size,
        };
    }

    // Compressed instructions
    if (inst & 0x3) != 0x3 {
        // Compressed instructions have lowest 2 bits != 11
        let compressed_op = (inst & 0xffff) as u16; // Only look at lower 16 bits
        let op_low = compressed_op & 0xe003; // Mask for opcode

        // C.SW: 110 uimm[5:3] rs1' uimm[2|6] rs2' 00
        // C.SD: 111 uimm[5:3] rs1' uimm[2|6] rs2' 00
        if (op_low & 0xC003) == 0xC000 {
            // C.SW/C.SD
            let is_sd = (compressed_op & 0x2000) != 0;
            let rs2 = (((compressed_op >> 2) & 0x7) + 8) as u8; // rs2' is encoded as x8-x15

            return MemInstructionType::RegStore {
                data_reg: rs2,
                size: if is_sd { 8 } else { 4 },
            };
        }

        // Stack-Pointer-Based Stores
        // C.SWSP: 110 uimm[5:2|7:6] rs2 01
        // C.SDSP: 111 uimm[5:2|7:6] rs2 01
        if (compressed_op & 0xC003) == 0xC002 {
            let is_sd = (compressed_op & 0x2000) != 0;
            let rs2 = ((compressed_op >> 2) & 0x1F) as u8; // rs2 is in bits 6:2

            return MemInstructionType::RegStore {
                data_reg: rs2,
                size: if is_sd { 8 } else { 4 },
            };
        }
    }

    MemInstructionType::NotStore
}

pub fn get_inst_data(mepc: usize) -> u32 {
    let inst = unsafe { *(mepc as *const u32) };
    inst
}

#[inline]
pub fn is_accessing_dma(mtval: usize) -> bool {
    let sm = sm();
    if sm.dma == 0 {
        return false;
    }
    dma::in_region(mtval, sm.dma)
}

pub fn get_reg_data(regs: &TrapRegs, inst: u32) -> (usize, usize) {
    let mem_type = analyze_instruction(inst);
    let (reg, size) = match mem_type {
        MemInstructionType::RegStore { data_reg, size } => (data_reg, size),
        _ => (0, 0),
    };
    (regs.get_reg(reg as usize), size as usize)
}

pub fn is_dma_enabled(data: usize) -> bool {
    // data > DMA_ENABLE_OFFSET & 0x1
    dma::CSRFlags::from_bits_truncate(data).contains(dma::CSRFlags::DMA_ENABLE)
}

pub fn check_dma_access() -> Result<(), Error> {
    todo!()
}

// pub fn check_dma_access() -> Result<(), Error> {
//     let eid = hart::current().get_enc().get_id();
//     let addr = sm().mmio.dma.ok_or(Error::Other("DMA not initialized"))?;
//     // let dma = dma::as_dma_controller(addr);

//     // get the size
//     let size = dma.get_size();

//     // check if the src address and dst address are valid
//     let src_addr = dma.get_src_addr();
//     let mut addr = src_addr;
//     while addr < src_addr + size {
//         let pma = sm()
//             .pma_mgr
//             .read()
//             .get_pma(addr)
//             .ok_or(Error::InvalidAddress(addr))?;
//         if pma.get_prop().get_owner() != eid {
//             return Err(Error::Other("DMA access denied"));
//         }
//         addr += 0x1000;
//     }

//     // check if the src address and dst address are valid
//     let dst_addr = dma.get_dst_addr();
//     let mut addr = dst_addr;
//     while addr < dst_addr + size {
//         let pma = sm()
//             .pma_mgr
//             .read()
//             .get_pma(addr)
//             .ok_or(Error::InvalidAddress(addr))?;
//         if pma.get_prop().get_owner() != eid {
//             return Err(Error::Other("DMA access denied"));
//         }
//         addr += 0x1000;
//     }

//     Ok(())
// }

#[allow(unused)]
pub fn write_dma_regs(data: usize, size: usize) {}
