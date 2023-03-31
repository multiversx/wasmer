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
    use wasmer_compiler_singlepass::*;
    use wasmer_types::{FunctionIndex, FunctionType, Type};
    use wasmer::vm::vmoffsets::VMOffsets;
    use wasmer::vm::VMOffsets;

    #[test]
    fn test_std_trampoline() {
        let signature = make_test_signature();

        let trampoline_arm_basic =
            gen_std_trampoline_arm64(&signature, CallingConvention::WasmBasicCAbi);
        println!("\nbasic trampoline body: {:?}", trampoline_arm_basic.body);

        let trampoline_arm_apple =
            gen_std_trampoline_arm64(&signature, CallingConvention::AppleAarch64);
        println!("\napple trampoline body: {:?}", trampoline_arm_apple.body);

        assert_ne!(trampoline_arm_basic, trampoline_arm_apple);
    }

    #[test]
    fn test_dynamic_import_trampoline() {
        let signature = make_test_signature();
        let vmoffsets = VMOffsets::new_for_trampolines(4);

        let trampoline_arm_basic =
            gen_std_dynamic_import_trampoline_arm64(&vmoffsets, &signature, CallingConvention::WasmBasicCAbi);
        println!("\nbasic trampoline body: {:?}", trampoline_arm_basic.body);

        let trampoline_arm_apple =
            gen_std_dynamic_import_trampoline_arm64(&vmoffsets, &signature, CallingConvention::AppleAarch64);
        println!("\napple trampoline body: {:?}", trampoline_arm_apple.body);

        assert_ne!(trampoline_arm_basic, trampoline_arm_apple);
    }

    fn make_test_signature() -> FunctionType {
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

        FunctionType::new(params, returns)
    }
}
