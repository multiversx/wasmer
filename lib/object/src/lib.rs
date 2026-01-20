//! Object creator for Wasm Compilations.
//!
//! Given a compilation result (this is, the result when calling `Compiler::compile_module`)
//! this exposes functions to create an Object file for a given target.

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

mod error;
mod module;

pub use crate::error::ObjectError;
pub use crate::module::{emit_compilation, emit_data, get_object_for_target};
