use crate::{
    error::{update_last_error, CApiError},
    instance::{wasmer_instance_t, wasmer_compilation_options_t, CompilationOptions, prepare_middleware_chain_generator, get_compiler},
    wasmer_result_t,
};
use wasmer_runtime_core::{cache::Artifact, import::ImportObject};
use std::slice;
use crate::import::GLOBAL_IMPORT_OBJECT;

#[cfg(not(feature = "cranelift-backend"))]
use wasmer_middleware_common::metering;

#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_cache(
    instance: *mut wasmer_instance_t,
    cache_bytes: *mut *const u8,
    cache_len: *mut u32,
) -> wasmer_result_t {
    if instance.is_null() {
        update_last_error(CApiError {
            msg: "null instance".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }

    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    let module = instance.module();
    match module.cache() {
        Err(error) => {
            update_last_error(CApiError {
                msg: format!("{:?}", error),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
        Ok(artifact) => {
            match artifact.serialize() {
                Err(error) => {
                    update_last_error(CApiError {
                        msg: format!("{:?}", error),
                    });
                    return wasmer_result_t::WASMER_ERROR;
                }
                Ok(bytes_vec) => {
                    if !bytes_vec.is_empty() {
                        let buf = bytes_vec.into_boxed_slice();
                        *cache_bytes = buf.as_ptr();
                        *cache_len = buf.len() as u32;
                        std::mem::forget(buf);
                    }
                }
            }
        }
    };

    wasmer_result_t::WASMER_OK
}

#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_from_cache(
    instance: *mut *mut wasmer_instance_t,
    cache_bytes: *mut u8,
    cache_len: u32,
    options: *const wasmer_compilation_options_t,
) -> wasmer_result_t {
    if cache_bytes.is_null() {
        update_last_error(CApiError {
            msg: "cache bytes ptr is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }

    let bytes: &[u8] = slice::from_raw_parts(cache_bytes, cache_len as usize);
    let options: &CompilationOptions = &*(options as *const CompilationOptions);
    let compiler_chain_generator = prepare_middleware_chain_generator(&options);
    let compiler = get_compiler(compiler_chain_generator);
    let new_module = match Artifact::deserialize(bytes) {
        Ok(serialized_cache) => match wasmer_runtime_core::load_cache_with(serialized_cache, &compiler) {
            Ok(deserialized_module) => {
                deserialized_module
            }
            Err(_) => {
                update_last_error(CApiError {
                    msg: "Failed to compile the serialized module".to_string(),
                });
                return wasmer_result_t::WASMER_ERROR;
            }
        },
        Err(err) => {
            println!("{:?}", err);
            update_last_error(CApiError {
                msg: "Failed to deserialize the module".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
    };

    let import_object: &mut ImportObject = &mut *(GLOBAL_IMPORT_OBJECT as *mut ImportObject);
    let result_instantiation = new_module.instantiate(&import_object);
    let mut new_instance = match result_instantiation {
        Ok(instance) => instance,
        Err(error) => {
            update_last_error(error);
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    metering::set_points_limit(&mut new_instance, options.gas_limit);
    *instance = Box::into_raw(Box::new(new_instance)) as *mut wasmer_instance_t;
    wasmer_result_t::WASMER_OK
}
