OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS {
    . = $SM_TEXT_START;
    .text : {
        *(.text.entry)
        *(.text.sbi)
        KEEP(*(.text.sbi));
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
        *(.bss, .bss.*)
    }
    .bss : {
    
    }
    /DISCARD/ : {
        *(.eh_frame)
    }
}
