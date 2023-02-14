use super::{initialize_globals, initialize_memories, initialize_passive_elements, Instance};
use crate::{InstanceHandle, MemoryError, Trap};
use wasmer_types::{DataInitializer, OwnedDataInitializer};

impl InstanceHandle {
    /// Resets the `[Globals`] and [`Memories`] for an [`Instance`].
    pub fn reset(&self, data_initializers: &[OwnedDataInitializer]) -> Result<(), String> {
        let now = std::time::Instant::now();
        let instance = self.instance.as_ref();
        reset_passive_elements(instance);
        reset_globals(instance);
        reset_memories(instance, data_initializers)?;
        println!("reset: {:?}", now.elapsed());
        Ok(())
    }
}

fn reset_passive_elements(instance: &Instance) {
    let now = std::time::Instant::now();
    initialize_passive_elements(instance);
    println!("reset_passive_elements: {:?}", now.elapsed());
}

fn reset_globals(instance: &Instance) {
    let now = std::time::Instant::now();
    initialize_globals(instance);
    println!("reset_globals: {:?}", now.elapsed());
}

fn reset_memories(
    instance: &Instance,
    data_initializers: &[OwnedDataInitializer],
) -> Result<(), String> {
    let now = std::time::Instant::now();
    zero_memories(instance)?;
    shrink_memories(instance)?;
    reinitialize_memories(instance, data_initializers)?;
    println!("reset_memories: {:?}", now.elapsed());
    Ok(())
}

fn zero_memories(instance: &Instance) -> Result<(), String> {
    let now = std::time::Instant::now();
    for (_local_memory_index, memory) in instance.memories.iter() {
        unsafe {
            let memory = memory.vmmemory().as_ref();
            let len = memory.current_length as u32;
            let result = memory.memory_fill(0, 0, len);
            if let Err(trap) = result {
                match trap {
                    Trap::Lib {
                        trap_code,
                        backtrace: _,
                    } => return Err(String::from(trap_code.message())),
                    _ => return Err(String::from("unexpected trap")),
                }
            }
        }
    }
    println!("zero_memories: {:?}", now.elapsed());
    Ok(())
}

fn shrink_memories(instance: &Instance) -> Result<(), String> {
    let now = std::time::Instant::now();
    for (_local_memory_index, memory) in instance.memories.iter() {
        println!("memory size before shrink: {:?}", memory.size());
        let result = memory.shrink_to_minimum();
        if let Err(memory_error) = result {
            match memory_error {
                MemoryError::Region(message) => return Err(message),
                MemoryError::InvalidMemory { reason } => return Err(reason),
                _ => return Err(String::from("unexpected memory error")),
            }
        }
        println!("memory size after shrink: {:?}", memory.size());
    }
    println!("shrink_memories: {:?}", now.elapsed());
    Ok(())
}

fn reinitialize_memories(
    instance: &Instance,
    data_initializers: &[OwnedDataInitializer],
) -> Result<(), String> {
    let now = std::time::Instant::now();
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
    println!("reinitialize_memories: {:?}", now.elapsed());
    Ok(())
}
