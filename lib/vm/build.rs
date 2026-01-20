//! Runtime build script compiles C code using setjmp for trap handling.

use std::env;

fn compile_handlers() {
    println!("cargo:rerun-if-changed=src/trap/handlers.c");

    cc::Build::new()
        .warnings(true)
        .define(
            &format!(
                "CFG_TARGET_OS_{}",
                env::var("CARGO_CFG_TARGET_OS").unwrap().to_uppercase()
            ),
            None,
        )
        .file("src/trap/handlers.c")
        .compile("handlers");
}

#[rustversion::since(1.89)]
fn configure_probestack() {
    println!("cargo::rustc-cfg=missing_rust_probestack");
    println!("cargo::rustc-check-cfg=cfg(missing_rust_probestack)");
}

#[rustversion::before(1.89)]
fn configure_probestack() {
    println!("cargo::rustc-check-cfg=cfg(missing_rust_probestack)");
}

fn main() {
    configure_probestack();
    compile_handlers();
}
