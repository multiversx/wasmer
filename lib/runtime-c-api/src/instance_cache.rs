use crate::{
    error::{update_last_error, CApiError},
    instance::wasmer_instance_t,
    wasmer_result_t,
};
// use wasmer_runtime::{
//     Instance, Module
// };
// use wasmer_runtime_core::cache::{Artifact, Error as CacheError};


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
    let cache_result = module.cache();
    match cache_result {
        Err(error) => {
            update_last_error(CApiError {
                msg: format!("{:?}", error),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
        Ok(artifact) => {
            let serialize_result = artifact.serialize();
            match serialize_result {
                Err(error) => {
                    update_last_error(CApiError {
                        msg: format!("{:?}", error),
                    });
                    return wasmer_result_t::WASMER_ERROR;
                }
                Ok(bytes_vec) => {
                    if !bytes_vec.is_empty() {
                        *cache_bytes = bytes_vec.as_ptr();
                        *cache_len = bytes_vec.len() as u32;
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
    _instance: *mut *mut wasmer_instance_t,
    _cache_bytes: *mut u8,
    _cache_len: u32,
) -> wasmer_result_t {
    // TODO set points limit
    wasmer_result_t::WASMER_OK
}
