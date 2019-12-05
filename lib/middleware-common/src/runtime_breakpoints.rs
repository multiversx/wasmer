use wasmer_runtime_core::{
    codegen::{Event, EventSink, FunctionMiddleware, InternalEvent},
    module::ModuleInfo,
    vm::{InternalField},
    wasmparser::{Operator, Type as WpType, TypeOrFuncType as WpTypeOrFuncType},
    Instance,
};

static RUNTIME_BREAKPOINT_VALUE: InternalField = InternalField::allocate();

#[derive(Copy, Clone, Debug)]
pub struct RuntimeBreakpointReachedError;


pub struct RuntimeBreakpointHandler {}

impl RuntimeBreakpointHandler {
    pub fn new() -> RuntimeBreakpointHandler {
        RuntimeBreakpointHandler {}
    }
}

impl FunctionMiddleware for RuntimeBreakpointHandler {
    type Error = String;
    fn feed_event<'a, 'b: 'a>(
        &mut self,
        op: Event<'a, 'b>,
        _module_info: &ModuleInfo,
        sink: &mut EventSink<'a, 'b>,
    ) -> Result<(), Self::Error> {
        match op {
            Event::Wasm(&ref op) | Event::WasmOwned(ref op) => {
                match *op {
                    Operator::Call { .. }
                    | Operator::CallIndirect { .. } => {
                        sink.push(Event::Internal(InternalEvent::GetInternal(
                            RUNTIME_BREAKPOINT_VALUE.index() as _,
                        )));
                        sink.push(Event::WasmOwned(Operator::I32Eqz));
                        sink.push(Event::WasmOwned(Operator::If {
                            ty: WpTypeOrFuncType::Type(WpType::EmptyBlockType),
                        }));
                        sink.push(Event::Internal(InternalEvent::Breakpoint(Box::new(|_| {
                            Err(Box::new(RuntimeBreakpointReachedError))
                        }))));
                        sink.push(Event::WasmOwned(Operator::End));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        sink.push(op);
        Ok(())
    }
}


pub fn set_runtime_breakpoint_value(instance: &mut Instance, value: u64) {
    instance.set_internal(&RUNTIME_BREAKPOINT_VALUE, value);
}
