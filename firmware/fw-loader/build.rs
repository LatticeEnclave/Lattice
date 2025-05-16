use std::{env, fs, path::PathBuf};

fn parser_hex_str(raw: &str) -> usize {
    let without_prefix = raw.trim_start_matches("0x");
    usize::from_str_radix(without_prefix, 16).unwrap()
}

fn prepare_lds() -> PathBuf {
    let raw = option_env!("FW_TEXT_START").unwrap_or("0x80000000");
    let text_start = parser_hex_str(raw);
    let next_addr = text_start + 0x20_0000;
    let mut lds = String::from_utf8(LINKER.to_vec()).unwrap();
    lds = lds.replace("$NEXT_ADDRESS", &format!("{next_addr:#x}"));
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, lds).unwrap();
    ld
}

fn main() {
    let ld = prepare_lds();
    println!("cargo:rustc-link-arg=-T{}", ld.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    println!("cargo:rerun-if-env-changed=FW_TEXT_START");
    println!("cargo:rerun-if-env-changed=SBI_SIZE");
    println!("cargo:rerun-if-env-changed=FW_PAYLOAD_PATH");
    println!("cargo:rerun-if-env-changed=TEE_SM_PATH");

    let payload_bin = if let Some(path) = std::env::var("FW_PAYLOAD_PATH").ok() {
        format!(r#".incbin "{path}""#)
    } else {
        "
        wfi
        j payload_bin
    "
        .to_owned()
    };

    let sm_bin = if let Some(path) = std::env::var("TEE_SM_PATH").ok() {
        format!(r#".incbin "{path}""#)
    } else {
        "
        wfi
        j sm_bin
    "
        .to_owned()
    };

    let asm_content = format!(
        r#"
    .section .payload, "ax", %progbits
        .align 4
        .globl payload_bin

payload_bin:
    {payload_bin}

    .section .sm, "ax", %progbits             
        .align 4
        .global sm_bin
sm_bin:
    {sm_bin}    
        "#,
    );

    let asm = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("payload.S");
    println!("cargo:rerun-if-changed={}", asm.display());
    if std::path::Path::new(asm).exists() {
        std::fs::remove_file(asm).unwrap();
    }

    std::fs::write(asm, asm_content).unwrap();
}

const LINKER: &[u8] = b"
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS {
    . = $NEXT_ADDRESS;
    .start : {
        PROVIDE(_loader_start = .);
        *(.start)
    }
    .payload : {
        . = ALIGN(0x10);
        PROVIDE( _payload_start = . );
        *(.payload)
        KEEP(*(.payload));
        PROVIDE( _payload_end = . );
    }
    .sm : {
        . = ALIGN(0x10);
        PROVIDE( _sm_start = . );
        *(.sm)
        KEEP(*(.sm));
        PROVIDE( _sm_end = . );
    }
    .text : {
        *(.text.entry)
        *(.text .text.*)
    } 
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        . = ALIGN(8);
        PROVIDE( _global_pointer = . + 0x800 );
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss.uninit)
        . = ALIGN(8);
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        . = ALIGN(8);
        ebss = .;
    }
    .stack : {
        . = ALIGN(0x10);
        PROVIDE( _loader_sp = . + 0x4000 );
    }
    /DISCARD/ : {
        *(.eh_frame)
    }
}";
