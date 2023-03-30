//! This test suite does all the tests that involve any compiler
//! implementation, such as: singlepass, cranelift or llvm depending
//! on what's available on the target.

#[macro_use]
extern crate compiler_test_derive;

mod config;
mod deterministic;
mod imports;
mod issues;
mod metering;
mod middlewares;
// mod multi_value_imports;
mod native_functions;
mod serialize;
mod traps;
mod wasi;
mod wast;

pub use crate::config::{Compiler, Config, Engine};
pub use crate::wasi::run_wasi;
pub use crate::wast::run_wast;
pub use wasmer_wast::WasiFileSystemKind;

#[cfg(feature = "singlepass")]
#[cfg(test)]
mod tests {
    use wasmer_compiler::CallingConvention;
    use wasmer_compiler_singlepass::gen_std_trampoline_arm64;
    use wasmer_types::{FunctionIndex, FunctionType, Type};

    #[test]
    fn just_run_this() {
        let params = vec![
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
            Type::I64,
        ];
        let returns = vec![Type::I32];

        let signature = FunctionType::new(params, returns);
        println!("signature: {:?}", signature);

        let trampoline_arm_basic =
            gen_std_trampoline_arm64(&signature, CallingConvention::WasmBasicCAbi);
        println!("trampoline body: {:?}", trampoline_arm_basic.body);

        let trampoline_arm_apple =
            gen_std_trampoline_arm64(&signature, CallingConvention::AppleAarch64);
        println!("trampoline body: {:?}", trampoline_arm_apple.body);

        assert_ne!(trampoline_arm_basic, trampoline_arm_apple);
    }
}
