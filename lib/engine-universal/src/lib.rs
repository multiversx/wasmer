//! Universal backend for Wasmer compilers.
//!
//! Given a compiler (such as `CraneliftCompiler` or `LLVMCompiler`)
//! it generates the compiled machine code, and publishes it into
//! memory so it can be used externally.

#![deny(missing_docs, trivial_numeric_casts, unused_extern_crates)]
#![warn(unused_import_braces)]
#![allow(clippy::new_without_default)]
#![warn(
    clippy::float_arithmetic,
    clippy::mut_mut,
    clippy::nonminimal_bool,
    clippy::option_map_unwrap_or,
    clippy::option_map_unwrap_or_else,
    clippy::print_stdout,
    clippy::unicode_not_nfc,
    clippy::use_self
)]

mod artifact;
mod builder;
mod code_memory;
mod engine;
mod link;
mod serialize;
mod trampoline;
mod unwind;

pub use crate::artifact::UniversalArtifact;
pub use crate::builder::Universal;
pub use crate::code_memory::CodeMemory;
pub use crate::engine::UniversalEngine;
pub use crate::link::link_module;

/// Version number of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
