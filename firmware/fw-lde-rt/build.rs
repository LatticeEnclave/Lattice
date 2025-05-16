use std::{env, fs, path::PathBuf};

fn main() {
    let ld = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(ld, LINKER).unwrap();
    println!("cargo:rustc-link-arg=-T{}", ld.display());
    println!("cargo:rerun-if-changed=build.rs");
    // env::set_var("RUSTFLAGS", "-C target-feature=+relax");
    // println!("cargp:rustc-link-arg=--omagic");
    // println!("cargo:rustc-flags=+relax");
}

const LINKER: &[u8] = b"
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS {
    . = 0xffffffff80000000;
    .start : {
        *(.start)
    }
    .tp : {
        . = ALIGN(0x10);
        PROVIDE( _tp_start = . );
        *(.tp)
    }
    . = ALIGN(0x1000);
    .text : {
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
    /DISCARD/ : {
        *(.eh_frame)
    }
}";
