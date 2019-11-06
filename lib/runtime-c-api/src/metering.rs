use crate::{
    export::{wasmer_import_export_kind},
    import::{wasmer_import_t},
    error::{update_last_error, CApiError},
    instance::wasmer_instance_t,
    module::wasmer_module_t,
    wasmer_result_t,
};
use libc::{c_int};
use std::{collections::HashMap, slice};
use wasmer_runtime::{Global, Memory, Table};
use wasmer_runtime_core::{
    export::Export,
    import::{ImportObject, Namespace},
};

#[cfg(feature = "metering")]
use wasmer_runtime_core::backend::Compiler;

#[cfg(not(feature = "cranelift-backend"))]
use wasmer_middleware_common::metering;

pub const OPCODE_COUNT: usize = 410;
static mut OPCODE_COSTS: [u32; OPCODE_COUNT] = [0; OPCODE_COUNT];


#[allow(clippy::cast_ptr_alignment)]
#[cfg(feature = "metering")]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instantiate_with_metering(
    instance: *mut *mut wasmer_instance_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
    imports: *mut wasmer_import_t,
    imports_len: c_int,
    gas_limit: u64,
    opcode_costs_pointer: *const u32,
) -> wasmer_result_t {
    if wasm_bytes.is_null() {
        update_last_error(CApiError {
            msg: "wasm bytes ptr is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }
    let imports: &[wasmer_import_t] = slice::from_raw_parts(imports, imports_len as usize);
    let mut import_object = ImportObject::new();
    let mut namespaces = HashMap::new();
    for import in imports {
        let module_name = slice::from_raw_parts(
            import.module_name.bytes,
            import.module_name.bytes_len as usize,
        );
        let module_name = if let Ok(s) = std::str::from_utf8(module_name) {
            s
        } else {
            update_last_error(CApiError {
                msg: "error converting module name to string".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        };
        let import_name = slice::from_raw_parts(
            import.import_name.bytes,
            import.import_name.bytes_len as usize,
        );
        let import_name = if let Ok(s) = std::str::from_utf8(import_name) {
            s
        } else {
            update_last_error(CApiError {
                msg: "error converting import_name to string".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        };

        let namespace = namespaces.entry(module_name).or_insert_with(Namespace::new);

        let export = match import.tag {
            wasmer_import_export_kind::WASM_MEMORY => {
                let mem = import.value.memory as *mut Memory;
                Export::Memory((&*mem).clone())
            }
            wasmer_import_export_kind::WASM_FUNCTION => {
                let func_export = import.value.func as *mut Export;
                (&*func_export).clone()
            }
            wasmer_import_export_kind::WASM_GLOBAL => {
                let global = import.value.global as *mut Global;
                Export::Global((&*global).clone())
            }
            wasmer_import_export_kind::WASM_TABLE => {
                let table = import.value.table as *mut Table;
                Export::Table((&*table).clone())
            }
        };
        namespace.insert(import_name, export);
    }
    for (module_name, namespace) in namespaces.into_iter() {
        import_object.register(module_name, namespace);
    }

    OPCODE_COSTS.copy_from_slice(slice::from_raw_parts(opcode_costs_pointer, OPCODE_COUNT));

    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    let compiler = get_metered_compiler(gas_limit);
    let result_compilation = wasmer_runtime_core::compile_with(bytes, &compiler);
    let new_module = match result_compilation {
        Ok(module) => module,
        Err(_) => {
            update_last_error(CApiError {
                msg: "compile error".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    let result_instantiation = new_module.instantiate(&import_object);
    let new_instance = match result_instantiation {
        Ok(instance) => instance,
        Err(error) => {
            update_last_error(error);
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    *instance = Box::into_raw(Box::new(new_instance)) as *mut wasmer_instance_t;
    wasmer_result_t::WASMER_OK
}

/// Creates a new Module with gas limit from the given wasm bytes.
///
/// Returns `wasmer_result_t::WASMER_OK` upon success.
///
/// Returns `wasmer_result_t::WASMER_ERROR` upon failure. Use `wasmer_last_error_length`
/// and `wasmer_last_error_message` to get an error message.
#[allow(clippy::cast_ptr_alignment)]
#[cfg(feature = "metering")]
#[no_mangle]
pub unsafe extern "C" fn wasmer_compile_with_gas_metering(
    module: *mut *mut wasmer_module_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
    gas_limit: u64,
    opcode_costs_pointer: *const u32,
) -> wasmer_result_t {
    if module.is_null() {
        update_last_error(CApiError {
            msg: "module is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }
    if wasm_bytes.is_null() {
        update_last_error(CApiError {
            msg: "wasm bytes is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }

    OPCODE_COSTS.copy_from_slice(slice::from_raw_parts(opcode_costs_pointer, OPCODE_COUNT));

    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    let compiler = get_metered_compiler(gas_limit);
    let result = wasmer_runtime_core::compile_with(bytes, &compiler);
    let new_module = match result {
        Ok(instance) => instance,
        Err(_) => {
            update_last_error(CApiError {
                msg: "compile error".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    *module = Box::into_raw(Box::new(new_module)) as *mut wasmer_module_t;
    wasmer_result_t::WASMER_OK
}

#[cfg(feature = "metering")]
unsafe fn get_metered_compiler(limit: u64) -> impl Compiler {
    use wasmer_runtime_core::codegen::{MiddlewareChain, StreamingCompiler};

    #[cfg(feature = "llvm-backend")]
    use wasmer_llvm_backend::ModuleCodeGenerator as MeteredMCG;

    #[cfg(feature = "singlepass-backend")]
    use wasmer_singlepass_backend::ModuleCodeGenerator as MeteredMCG;

    #[cfg(feature = "cranelift-backend")]
    use wasmer_clif_backend::CraneliftModuleCodeGenerator as MeteredMCG;

    let c: StreamingCompiler<MeteredMCG, _, _, _, _> = StreamingCompiler::new(move || {
        let mut chain = MiddlewareChain::new();

        chain.push(metering::Metering::new(limit, &OPCODE_COSTS));
        chain
    });
    c
}

// returns gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(feature = "metering")]
pub unsafe extern "C" fn wasmer_instance_get_points_used(instance: *mut wasmer_instance_t) -> u64 {
    if instance.is_null() {
        return 0;
    }
    let instance = &*(instance as *const wasmer_runtime::Instance);
    let points = metering::get_points_used(instance);
    points
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(feature = "metering")]
pub unsafe extern "C" fn wasmer_instance_set_points_used(
    instance: *mut wasmer_instance_t,
    new_gas: u64,
) {
    if instance.is_null() {
        return;
    }
    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    metering::set_points_used(instance, new_gas)
}

/*** placeholder implementation if metering feature off ***/

// Without metering, wasmer_compile_with_gas_metering is a copy of wasmer_compile
#[cfg(not(feature = "metering"))]
#[no_mangle]
pub unsafe extern "C" fn wasmer_compile_with_gas_metering(
    module: *mut *mut wasmer_module_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
    _: u64,
) -> wasmer_result_t {
    if module.is_null() {
        update_last_error(CApiError {
            msg: "module is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }
    if wasm_bytes.is_null() {
        update_last_error(CApiError {
            msg: "wasm bytes is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }

    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    // TODO: this implicitly uses default_compiler() is that proper? maybe we override default_compiler
    let result = wasmer_runtime::compile(bytes);
    let new_module = match result {
        Ok(instance) => instance,
        Err(error) => {
            update_last_error(error);
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    *module = Box::into_raw(Box::new(new_module)) as *mut wasmer_module_t;
    wasmer_result_t::WASMER_OK
}

// returns gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(not(feature = "metering"))]
pub unsafe extern "C" fn wasmer_instance_get_points_used(_: *mut wasmer_instance_t) -> u64 {
    0
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(not(feature = "metering"))]
pub unsafe extern "C" fn wasmer_instance_set_points_used(_: *mut wasmer_instance_t, _: u64) {}
