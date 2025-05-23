use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Token, parse_macro_input, punctuated::Punctuated};

fn parser_hex_str(raw: &str) -> usize {
    let without_prefix = raw.trim_start_matches("0x");
    usize::from_str_radix(without_prefix, 16).unwrap()
}

fn parser_dec_str(raw: &str) -> usize {
    usize::from_str_radix(raw, 10).unwrap()
}

#[proc_macro]
pub fn usize_env_or(input: TokenStream) -> TokenStream {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let exprs = parse_macro_input!(input with parser);
    let env_var = match &exprs[0] {
        syn::Expr::Lit(expr_lit) => {
            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                lit_str.value()
            } else {
                panic!("First argument must be a string literal (environment variable name)");
            }
        }
        _ => panic!("First argument must be a string literal (environment variable name)"),
    };
    let default_value = match &exprs[1] {
        syn::Expr::Lit(expr_lit) => {
            if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                lit_int.base10_parse::<usize>().unwrap()
            } else {
                panic!("Second argument must be an integer literal (default value)");
            }
        }
        _ => panic!("Second argument must be an integer literal (default value)"),
    };

    // 读取环境变量
    let value = std::env::var(&env_var)
        .ok()
        .map(|v| {
            if v.starts_with("0x") {
                parser_hex_str(&v)
            } else {
                parser_dec_str(&v)
            }
        })
        .unwrap_or(default_value);

    // 生成代码
    let output = quote! {
        #value
    };

    output.into()
}
