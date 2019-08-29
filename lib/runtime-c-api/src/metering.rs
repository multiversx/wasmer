use crate::{
    error::{update_last_error},
    instance::{wasmer_instance_t},
    module::{wasmer_module_t},
    wasmer_result_t,
};
use std::{slice};


#[cfg(feature = "metering")]
use wasmer_middleware_common::metering;
use wasmer_runtime_core::backend::{Compiler};
use wasmer_runtime_core::{compile_with};
//use wasmer_runtime::{Instance};

/// Creates a new Module with gas limit from the given wasm bytes.
///
/// Returns `wasmer_result_t::WASMER_OK` upon success.
///
/// Returns `wasmer_result_t::WASMER_ERROR` upon failure. Use `wasmer_last_error_length`
/// and `wasmer_last_error_message` to get an error message.
#[allow(clippy::cast_ptr_alignment)]
#[cfg(feature = "metering")]
#[no_mangle]
pub unsafe extern "C" fn wasmer_compile_with_limit(
    module: *mut *mut wasmer_module_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
    gas_limit: u32, // TODO: allow more than 4 billion?
) -> wasmer_result_t {
    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    let result = compile_with(bytes, &get_metered_compiler(gas_limit as u64));
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

#[cfg(feature = "metering")]
fn get_metered_compiler(limit: u64) -> impl Compiler {
    use wasmer_runtime_core::codegen::{MiddlewareChain, StreamingCompiler};
    use wasmer_singlepass_backend::ModuleCodeGenerator as SinglePassMCG;
    let c: StreamingCompiler<SinglePassMCG, _, _, _, _> = StreamingCompiler::new(move || {
        let mut chain = MiddlewareChain::new();
        chain.push(metering::Metering::new(limit));
        chain
    });
    c
}

// returns gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(feature = "metering")]
pub unsafe extern "C" fn wasmer_instance_get_points_used(
    _: *mut wasmer_instance_t,
) ->  u32 { // TODO: return u64
    0
    // TODO
//    let instance_ref = &*(instance as *const Instance);
//    let points = metering::get_points_used(instance);
//    points as u32
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(feature = "metering")]
pub unsafe extern "C" fn wasmer_instance_set_points_used(
    _: *mut wasmer_instance_t,
    _: u32,
) {
    // TODO
//    let instance_ref = &*(instance as *const Instance);
//    metering::set_points_used(instance, new_gas as u64)
}


/*** placeholder implementation if metering feature off ***/
//

// Without metering, wasmer_compile_with_limit is a copy of wasmer_compile
#[cfg(not(feature = "metering"))]
#[no_mangle]
pub unsafe extern "C" fn wasmer_compile_with_limit(
    module: *mut *mut wasmer_module_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
    _: u32, // TODO: allow more than 4 billion?
) -> wasmer_result_t {
    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    let result = compile(bytes);
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
pub unsafe extern "C" fn wasmer_instance_get_points_used(
    _: *mut wasmer_instance_t,
) ->  u32 {
    0
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
#[cfg(not(feature = "metering"))]
pub unsafe extern "C" fn wasmer_instance_set_points_used(
    _: *mut wasmer_instance_t,
    _: u32,
) { }