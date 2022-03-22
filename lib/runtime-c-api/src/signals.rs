use wasmer_runtime_core::fault::SIGSEGV_PASSTHROUGH;
use std::sync::atomic::Ordering;

#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_set_sigsegv_passthrough() {
    SIGSEGV_PASSTHROUGH.swap(true, Ordering::SeqCst);
}
