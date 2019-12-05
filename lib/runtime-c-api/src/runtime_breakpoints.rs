use crate::instance::wasmer_instance_t;

use wasmer_middleware_common::runtime_breakpoints::set_runtime_breakpoint_value;

#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_set_runtime_breakpoint_value(
    instance: *mut wasmer_instance_t,
    value: u64,
) {
    if instance.is_null() {
        return;
    }
    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    set_runtime_breakpoint_value(instance, value);
}
