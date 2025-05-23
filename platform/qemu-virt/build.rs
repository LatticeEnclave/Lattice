use std::{env, fs, path::PathBuf};

fn parser_hex_str(raw: &str) -> usize {
    let without_prefix = raw.trim_start_matches("0x");
    usize::from_str_radix(without_prefix, 16).unwrap()
}

fn main() {
    let ld = prepare_lds();
    println!("cargo:rustc-link-arg=-T{}", ld.display());
    println!("cargo:rerun-if-env-changed=SM_TEXT_START");
}

fn prepare_lds() -> PathBuf {
    let sm_start: usize = parser_hex_str(option_env!("SM_TEXT_START").unwrap_or("0x0"));
    let mut lds = LINKER.to_owned();
    lds = lds.replace("$SM_TEXT_START", &format!("{:#x}", sm_start));
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, lds).unwrap();
    ld
}

const LINKER: &str = include_str!("linker.ld");
