use super::{initialize_globals, initialize_memories, initialize_passive_elements, Instance};
use crate::{InstanceHandle, MemoryError, Trap};
use wasmer_types::{DataInitializer, OwnedDataInitializer};

impl InstanceHandle {
    /// Resets the `[Globals`] and [`Memories`] for an [`Instance`].
    pub fn reset(&self, data_initializers: &[OwnedDataInitializer]) -> Result<(), String> {
        let instance = self.instance.as_ref();
        reset_passive_elements(instance);
        reset_globals(instance);
        reset_memories(instance, data_initializers)
    }
}

fn reset_passive_elements(instance: &Instance) {
    initialize_passive_elements(instance);
}

fn reset_globals(instance: &Instance) {
    initialize_globals(instance);
}

fn reset_memories(
    instance: &Instance,
    data_initializers: &[OwnedDataInitializer],
) -> Result<(), String> {
    // `shrink_to_minimum` discards everything above the minimum and zeroes
    // the rest, so the memories only need their data segments re-applied.
    shrink_memories(instance)?;
    reinitialize_memories(instance, data_initializers)
}

fn shrink_memories(instance: &Instance) -> Result<(), String> {
    for (_local_memory_index, memory) in instance.memories.iter() {
        let result = memory.shrink_to_minimum();
        if let Err(memory_error) = result {
            match memory_error {
                MemoryError::Region(message) => return Err(message),
                MemoryError::InvalidMemory { reason } => return Err(reason),
                _ => return Err(String::from("unexpected memory error")),
            }
        }
    }

    Ok(())
}

fn reinitialize_memories(
    instance: &Instance,
    data_initializers: &[OwnedDataInitializer],
) -> Result<(), String> {
    let data_initializers = data_initializers
        .iter()
        .map(|init| DataInitializer {
            location: init.location.clone(),
            data: &*init.data,
        })
        .collect::<Vec<_>>();
    let result = initialize_memories(instance, &data_initializers);
    if let Err(trap) = result {
        match trap {
            Trap::Lib {
                trap_code,
                backtrace: _,
            } => return Err(String::from(trap_code.message())),
            _ => return Err(String::from("unexpected trap")),
        }
    }

    Ok(())
}
