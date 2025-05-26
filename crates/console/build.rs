fn main() {
    println!("cargo:rustc-flag=no-default-features");
    println!("cargo:rerun-if-env-changed=LOG");
    let level = std::env::var("LOG");
    eprintln!("{level:?}");
    match std::env::var("LOG").unwrap_or("info".to_string()).as_str() {
        "error" | "ERROR" => log_feature("error"),
        "info" | "INFO" => log_feature("info"),
        "debug" | "DEBUG" => log_feature("debug"),
        "trace" | "TRACE" => log_feature("trace"),
        _ => log_feature("info"),
    }
}

fn log_feature(level: &str) {
    println!("cargo:rustc-cfg=feature=\"{level}\"")
}
