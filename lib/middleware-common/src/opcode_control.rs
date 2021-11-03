use wasmer_runtime_core::{
    codegen::{Event, EventSink, FunctionMiddleware, InternalEvent},
    wasmparser::{Operator, Type as WpType, TypeOrFuncType as WpTypeOrFuncType},
    vm::InternalField,
    module::ModuleInfo,
};

use crate::runtime_breakpoints::{push_runtime_breakpoint, BREAKPOINT_VALUE_EXECUTION_FAILED};

static FIELD_OPERAND_BACKUP: InternalField = InternalField::allocate();

pub struct OpcodeControl {
    pub max_memory_grow_delta: usize,
}

impl OpcodeControl {
    pub fn new(max_memory_grow_delta: usize) -> OpcodeControl {
        OpcodeControl {
            max_memory_grow_delta,
        }
    }
}

impl FunctionMiddleware for OpcodeControl {
    type Error = String;
    fn feed_event<'a, 'b: 'a>(
        &mut self,
        op: Event<'a, 'b>,
        _: &ModuleInfo,
        sink: &mut EventSink<'a, 'b>,
        _: u32,
    ) -> Result<(), Self::Error> {
        match op {
            Event::Wasm(&ref op) | Event::WasmOwned(ref op) => {
                match *op {
                    Operator::MemoryGrow { reserved } => {
                        if reserved != 0 {
                            return Err("MemoryGrow must have memory index 0".to_string());
                        }

                        // Backup the top of the stack (the parameter for memory.grow) in order to
                        // duplicate it: once for the comparison against max_memory_grow_delta and
                        // again for memory.grow itself, assuming the comparison passes.
                        sink.push(Event::Internal(InternalEvent::SetInternal(
                            FIELD_OPERAND_BACKUP.index() as _,
                        )));

                        // Set up the comparison against max_memory_grow_delta.
                        sink.push(Event::Internal(InternalEvent::GetInternal(
                            FIELD_OPERAND_BACKUP.index() as _,
                        )));
                        sink.push(Event::WasmOwned(Operator::I32Const {
                            value: self.max_memory_grow_delta as i32,
                        }));
                        sink.push(Event::WasmOwned(Operator::I32GtU));
                        sink.push(Event::WasmOwned(Operator::If {
                            ty: WpTypeOrFuncType::Type(WpType::EmptyBlockType),
                        }));
                        push_runtime_breakpoint(sink, BREAKPOINT_VALUE_EXECUTION_FAILED);
                        sink.push(Event::WasmOwned(Operator::End));

                        // Bring back the backed-up operand for memory.grow.
                        sink.push(Event::Internal(InternalEvent::GetInternal(
                            FIELD_OPERAND_BACKUP.index() as _,
                        )));
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
