// This file contains code from external sources.
// Attributions: https://github.com/wasmerio/wasmer/blob/master/ATTRIBUTIONS.md

//! Memory management for linear memories.
//!
//! `LinearMemory` is to WebAssembly linear memories what `Table` is to WebAssembly tables.

use crate::mmap::Mmap;
use crate::vmcontext::VMMemoryDefinition;
use loupe::MemoryUsage;
use more_asserts::assert_ge;
#[cfg(feature = "enable-rkyv")]
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::cell::UnsafeCell;
use std::convert::TryInto;
use std::fmt;
use std::ptr::NonNull;
use std::sync::Mutex;
use thiserror::Error;
use wasmer_types::{Bytes, MemoryType, Pages};

/// Error type describing things that can go wrong when operating on Wasm Memories.
#[derive(Error, Debug, Clone, PartialEq, Hash)]
pub enum MemoryError {
    /// Low level error with mmap.
    #[error("Error when allocating memory: {0}")]
    Region(String),
    /// The operation would cause the size of the memory to exceed the maximum or would cause
    /// an overflow leading to unindexable memory.
    #[error("The memory could not grow: current size {} pages, requested increase: {} pages", current.0, attempted_delta.0)]
    CouldNotGrow {
        /// The current size in pages.
        current: Pages,
        /// The attempted amount to grow by in pages.
        attempted_delta: Pages,
    },
    /// The operation would cause the size of the memory size exceed the maximum.
    #[error("The memory is invalid because {}", reason)]
    InvalidMemory {
        /// The reason why the provided memory is invalid.
        reason: String,
    },
    /// Caller asked for more minimum memory than we can give them.
    #[error("The minimum requested ({} pages) memory is greater than the maximum allowed memory ({} pages)", min_requested.0, max_allowed.0)]
    MinimumMemoryTooLarge {
        /// The number of pages requested as the minimum amount of memory.
        min_requested: Pages,
        /// The maximum amount of memory we can allocate.
        max_allowed: Pages,
    },
    /// Caller asked for a maximum memory greater than we can give them.
    #[error("The maximum requested memory ({} pages) is greater than the maximum allowed memory ({} pages)", max_requested.0, max_allowed.0)]
    MaximumMemoryTooLarge {
        /// The number of pages requested as the maximum amount of memory.
        max_requested: Pages,
        /// The number of pages requested as the maximum amount of memory.
        max_allowed: Pages,
    },
    /// A user defined error value, used for error cases not listed above.
    #[error("A user-defined error occurred: {0}")]
    Generic(String),
}

/// Implementation styles for WebAssembly linear memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, MemoryUsage)]
#[cfg_attr(
    feature = "enable-rkyv",
    derive(RkyvSerialize, RkyvDeserialize, Archive)
)]
pub enum MemoryStyle {
    /// The actual memory can be resized and moved.
    Dynamic {
        /// Our chosen offset-guard size.
        ///
        /// It represents the size in bytes of extra guard pages after the end
        /// to optimize loads and stores with constant offsets.
        offset_guard_size: u64,
    },
    /// Address space is allocated up front.
    Static {
        /// The number of mapped and unmapped pages.
        bound: Pages,
        /// Our chosen offset-guard size.
        ///
        /// It represents the size in bytes of extra guard pages after the end
        /// to optimize loads and stores with constant offsets.
        offset_guard_size: u64,
    },
}

impl MemoryStyle {
    /// Returns the offset-guard size
    pub fn offset_guard_size(&self) -> u64 {
        match self {
            Self::Dynamic { offset_guard_size } => *offset_guard_size,
            Self::Static {
                offset_guard_size, ..
            } => *offset_guard_size,
        }
    }
}

/// Trait for implementing Wasm Memory used by Wasmer.
pub trait Memory: fmt::Debug + Send + Sync + MemoryUsage {
    /// Returns the memory type for this memory.
    fn ty(&self) -> MemoryType;

    /// Returns the memory style for this memory.
    fn style(&self) -> &MemoryStyle;

    /// Returns the number of allocated wasm pages.
    fn size(&self) -> Pages;

    /// Grow memory by the specified amount of wasm pages.
    fn grow(&self, delta: Pages) -> Result<Pages, MemoryError>;

    /// Return a [`VMMemoryDefinition`] for exposing the memory to compiled wasm code.
    ///
    /// The pointer returned in [`VMMemoryDefinition`] must be valid for the lifetime of this memory.
    fn vmmemory(&self) -> NonNull<VMMemoryDefinition>;

    /// Shrink memory to the minimum amount of wasm pages and zero its contents.
    ///
    /// Pages above the minimum are discarded and made inaccessible again, and
    /// the pages below it are zero-filled. For static memories the address
    /// space reservation and the base pointer are preserved.
    fn shrink_to_minimum(&self) -> Result<(), MemoryError>;
}

/// A linear memory instance.
#[derive(Debug, MemoryUsage)]
pub struct LinearMemory {
    // The underlying allocation.
    mmap: Mutex<WasmMmap>,

    minimum: Pages,

    // The optional maximum size in wasm pages of this linear memory.
    maximum: Option<Pages>,

    /// The WebAssembly linear memory description.
    memory: MemoryType,

    /// Our chosen implementation style.
    style: MemoryStyle,

    // Size in bytes of extra guard pages after the end to optimize loads and stores with
    // constant offsets.
    offset_guard_size: usize,

    /// The owned memory definition used by the generated code
    vm_memory_definition: VMMemoryDefinitionOwnership,

    // Records whether we're using a bounds-checking strategy which requires
    // handlers to catch trapping accesses.
    pub(crate) needs_signal_handlers: bool,
}

/// A type to help manage who is responsible for the backing memory of them
/// `VMMemoryDefinition`.
#[derive(Debug, MemoryUsage)]
enum VMMemoryDefinitionOwnership {
    /// The `VMMemoryDefinition` is owned by the `Instance` and we should use
    /// its memory. This is how a local memory that's exported should be stored.
    VMOwned(NonNull<VMMemoryDefinition>),
    /// The `VMMemoryDefinition` is owned by the host and we should manage its
    /// memory. This is how an imported memory that doesn't come from another
    /// Wasm module should be stored.
    HostOwned(Box<UnsafeCell<VMMemoryDefinition>>),
}

/// We must implement this because of `VMMemoryDefinitionOwnership::VMOwned`.
/// This is correct because synchronization of memory accesses is controlled
/// by the VM.
// REVIEW: I don't believe ^; this probably shouldn't be `Send`...
// mutations from other threads into this data could be a problem, but we probably
// don't want to use atomics for this in the generated code.
// TODO:
unsafe impl Send for LinearMemory {}

/// This is correct because all internal mutability is protected by a mutex.
unsafe impl Sync for LinearMemory {}

#[derive(Debug, MemoryUsage)]
struct WasmMmap {
    // Our OS allocation of mmap'd memory.
    alloc: Mmap,
    // The current logical size in wasm pages of this linear memory.
    size: Pages,
}

impl LinearMemory {
    /// Create a new linear memory instance with specified minimum and maximum number of wasm pages.
    ///
    /// This creates a `LinearMemory` with owned metadata: this can be used to create a memory
    /// that will be imported into Wasm modules.
    pub fn new(memory: &MemoryType, style: &MemoryStyle) -> Result<Self, MemoryError> {
        unsafe { Self::new_internal(memory, style, None) }
    }

    /// Create a new linear memory instance with specified minimum and maximum number of wasm pages.
    ///
    /// This creates a `LinearMemory` with metadata owned by a VM, pointed to by
    /// `vm_memory_location`: this can be used to create a local memory.
    ///
    /// # Safety
    /// - `vm_memory_location` must point to a valid location in VM memory.
    pub unsafe fn from_definition(
        memory: &MemoryType,
        style: &MemoryStyle,
        vm_memory_location: NonNull<VMMemoryDefinition>,
    ) -> Result<Self, MemoryError> {
        Self::new_internal(memory, style, Some(vm_memory_location))
    }

    /// Build a `LinearMemory` with either self-owned or VM owned metadata.
    unsafe fn new_internal(
        memory: &MemoryType,
        style: &MemoryStyle,
        vm_memory_location: Option<NonNull<VMMemoryDefinition>>,
    ) -> Result<Self, MemoryError> {
        if memory.minimum > Pages::max_value() {
            return Err(MemoryError::MinimumMemoryTooLarge {
                min_requested: memory.minimum,
                max_allowed: Pages::max_value(),
            });
        }
        // `maximum` cannot be set to more than `65536` pages.
        if let Some(max) = memory.maximum {
            if max > Pages::max_value() {
                return Err(MemoryError::MaximumMemoryTooLarge {
                    max_requested: max,
                    max_allowed: Pages::max_value(),
                });
            }
            if max < memory.minimum {
                return Err(MemoryError::InvalidMemory {
                    reason: format!(
                        "the maximum ({} pages) is less than the minimum ({} pages)",
                        max.0, memory.minimum.0
                    ),
                });
            }
        }

        let offset_guard_bytes = style.offset_guard_size() as usize;

        // If we have an offset guard, or if we're doing the static memory
        // allocation strategy, we need signal handlers to catch out of bounds
        // acceses.
        let needs_signal_handlers = offset_guard_bytes > 0
            || match style {
                MemoryStyle::Dynamic { .. } => false,
                MemoryStyle::Static { .. } => true,
            };

        let minimum_pages = match style {
            MemoryStyle::Dynamic { .. } => memory.minimum,
            MemoryStyle::Static { bound, .. } => {
                assert_ge!(*bound, memory.minimum);
                *bound
            }
        };
        let minimum_bytes = minimum_pages.bytes().0;
        let request_bytes = minimum_bytes.checked_add(offset_guard_bytes).unwrap();
        let mapped_pages = memory.minimum;
        let mapped_bytes = mapped_pages.bytes();

        let mut mmap = WasmMmap {
            alloc: Mmap::accessible_reserved(mapped_bytes.0, request_bytes)
                .map_err(MemoryError::Region)?,
            size: memory.minimum,
        };

        let base_ptr = mmap.alloc.as_mut_ptr();
        let mem_length = memory.minimum.bytes().0.try_into().unwrap();
        Ok(Self {
            mmap: Mutex::new(mmap),
            minimum: memory.minimum,
            maximum: memory.maximum,
            offset_guard_size: offset_guard_bytes,
            needs_signal_handlers,
            vm_memory_definition: if let Some(mem_loc) = vm_memory_location {
                {
                    let mut ptr = mem_loc;
                    let md = ptr.as_mut();
                    md.base = base_ptr;
                    md.current_length = mem_length;
                }
                VMMemoryDefinitionOwnership::VMOwned(mem_loc)
            } else {
                VMMemoryDefinitionOwnership::HostOwned(Box::new(UnsafeCell::new(
                    VMMemoryDefinition {
                        base: base_ptr,
                        current_length: mem_length,
                    },
                )))
            },
            memory: *memory,
            style: style.clone(),
        })
    }

    /// Get the `VMMemoryDefinition`.
    ///
    /// # Safety
    /// - You must ensure that you have mutually exclusive access before calling
    ///   this function. You can get this by locking the `mmap` mutex.
    unsafe fn get_vm_memory_definition(&self) -> NonNull<VMMemoryDefinition> {
        match &self.vm_memory_definition {
            VMMemoryDefinitionOwnership::VMOwned(ptr) => *ptr,
            VMMemoryDefinitionOwnership::HostOwned(boxed_ptr) => {
                NonNull::new_unchecked(boxed_ptr.get())
            }
        }
    }
}

impl Memory for LinearMemory {
    /// Returns the type for this memory.
    fn ty(&self) -> MemoryType {
        let minimum = self.size();
        let mut out = self.memory.clone();
        out.minimum = minimum;

        out
    }

    /// Returns the memory style for this memory.
    fn style(&self) -> &MemoryStyle {
        &self.style
    }

    /// Returns the number of allocated wasm pages.
    fn size(&self) -> Pages {
        // TODO: investigate this function for race conditions
        unsafe {
            let md_ptr = self.get_vm_memory_definition();
            let md = md_ptr.as_ref();
            Bytes::from(md.current_length).try_into().unwrap()
        }
    }

    /// Grow memory by the specified amount of wasm pages.
    ///
    /// Returns `None` if memory can't be grown by the specified amount
    /// of wasm pages.
    fn grow(&self, delta: Pages) -> Result<Pages, MemoryError> {
        let mut mmap_guard = self.mmap.lock().unwrap();
        let mmap = mmap_guard.borrow_mut();
        // Optimization of memory.grow 0 calls.
        if delta.0 == 0 {
            return Ok(mmap.size);
        }

        let new_pages = mmap
            .size
            .checked_add(delta)
            .ok_or(MemoryError::CouldNotGrow {
                current: mmap.size,
                attempted_delta: delta,
            })?;
        let prev_pages = mmap.size;

        if let Some(maximum) = self.maximum {
            if new_pages > maximum {
                return Err(MemoryError::CouldNotGrow {
                    current: mmap.size,
                    attempted_delta: delta,
                });
            }
        }

        // Wasm linear memories are never allowed to grow beyond what is
        // indexable. If the memory has no maximum, enforce the greatest
        // limit here.
        if new_pages >= Pages::max_value() {
            // Linear memory size would exceed the index range.
            return Err(MemoryError::CouldNotGrow {
                current: mmap.size,
                attempted_delta: delta,
            });
        }

        let delta_bytes = delta.bytes().0;
        let prev_bytes = prev_pages.bytes().0;
        let new_bytes = new_pages.bytes().0;

        if new_bytes > mmap.alloc.len() - self.offset_guard_size {
            // If the new size is within the declared maximum, but needs more memory than we
            // have on hand, it's a dynamic heap and it can move.
            let guard_bytes = self.offset_guard_size;
            let request_bytes =
                new_bytes
                    .checked_add(guard_bytes)
                    .ok_or_else(|| MemoryError::CouldNotGrow {
                        current: new_pages,
                        attempted_delta: Bytes(guard_bytes).try_into().unwrap(),
                    })?;

            let mut new_mmap =
                Mmap::accessible_reserved(new_bytes, request_bytes).map_err(MemoryError::Region)?;

            let copy_len = mmap.alloc.len() - self.offset_guard_size;
            new_mmap.as_mut_slice()[..copy_len].copy_from_slice(&mmap.alloc.as_slice()[..copy_len]);

            mmap.alloc = new_mmap;
        } else if delta_bytes > 0 {
            // Make the newly allocated pages accessible.
            mmap.alloc
                .make_accessible(prev_bytes, delta_bytes)
                .map_err(MemoryError::Region)?;
        }

        mmap.size = new_pages;

        // update memory definition
        unsafe {
            let mut md_ptr = self.get_vm_memory_definition();
            let md = md_ptr.as_mut();
            md.current_length = new_pages.bytes().0.try_into().unwrap();
            md.base = mmap.alloc.as_mut_ptr() as _;
        }

        Ok(prev_pages)
    }

    /// Return a `VMMemoryDefinition` for exposing the memory to compiled wasm code.
    fn vmmemory(&self) -> NonNull<VMMemoryDefinition> {
        let _mmap_guard = self.mmap.lock().unwrap();
        unsafe { self.get_vm_memory_definition() }
    }

    /// Shrink the memory back to its declared minimum size and zero it.
    ///
    /// Everything above `minimum` is discarded and made inaccessible again;
    /// the pages below it are zero-filled. Callers are expected to re-apply
    /// the module's data segments afterwards.
    ///
    /// For [`MemoryStyle::Static`] memories the whole `bound + guard`
    /// reservation is kept and the base address never changes. Compiled code
    /// for static memories carries no explicit bounds checks: it relies on
    /// every address a wasm32 program can form landing inside that
    /// reservation and hitting an inaccessible page. Re-mapping a smaller
    /// region here would silently remove that protection for the rest of the
    /// instance's life, so only the accessible prefix is changed, in place.
    ///
    /// [`MemoryStyle::Dynamic`] memories are bounds checked by compiled code
    /// and are already allowed to move on `grow`, so a fresh `minimum + guard`
    /// mapping is allocated for them.
    fn shrink_to_minimum(&self) -> Result<(), MemoryError> {
        let mut mmap_guard = self.mmap.lock().unwrap();
        let mmap = mmap_guard.borrow_mut();

        let new_pages = self.minimum;
        let new_bytes = new_pages.bytes().0;
        let current_bytes = mmap.size.bytes().0;

        match self.style {
            MemoryStyle::Static { .. } => {
                // `new_internal` guarantees `minimum <= bound`, so the whole
                // `[new_bytes, current_bytes)` range lies inside the
                // reservation and `[0, new_bytes)` is committed.
                if current_bytes > new_bytes {
                    mmap.alloc
                        .decommit(new_bytes, current_bytes - new_bytes)
                        .map_err(MemoryError::Region)?;
                }
                unsafe {
                    std::ptr::write_bytes(mmap.alloc.as_mut_ptr(), 0, new_bytes);
                }
            }
            MemoryStyle::Dynamic { .. } => {
                if current_bytes != new_bytes {
                    // Same request as `new_internal`, which already succeeded once.
                    let request_bytes = new_bytes.checked_add(self.offset_guard_size).unwrap();
                    mmap.alloc = Mmap::accessible_reserved(new_bytes, request_bytes)
                        .map_err(MemoryError::Region)?;
                } else {
                    unsafe {
                        std::ptr::write_bytes(mmap.alloc.as_mut_ptr(), 0, new_bytes);
                    }
                }
            }
        }

        mmap.size = new_pages;

        // update memory definition
        unsafe {
            let mut md_ptr = self.get_vm_memory_definition();
            let md = md_ptr.as_mut();
            md.current_length = new_bytes.try_into().unwrap();
            md.base = mmap.alloc.as_mut_ptr();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 64-bit production tunables: a 4 GiB bound plus a 2 GiB guard.
    fn static_style() -> MemoryStyle {
        MemoryStyle::Static {
            bound: Pages(0x1_0000),
            offset_guard_size: 0x8000_0000,
        }
    }

    fn reserved_len(memory: &LinearMemory) -> usize {
        memory.mmap.lock().unwrap().alloc.len()
    }

    fn assert_inaccessible(base: *mut u8, offset: usize) {
        let region = region::query(unsafe { base.add(offset) }).unwrap();
        assert_eq!(
            region.protection(),
            region::Protection::NONE,
            "offset {:#x} must be inaccessible",
            offset
        );
    }

    #[test]
    fn static_shrink_keeps_reservation_and_base() {
        let page = Pages(1).bytes().0;
        let bound = Pages(0x1_0000).bytes().0;
        let ty = MemoryType::new(Pages(2), Some(Pages(64)), false);
        let memory = LinearMemory::new(&ty, &static_style()).unwrap();
        let md_ptr = memory.vmmemory();
        let base = unsafe { md_ptr.as_ref().base };
        let reserved = reserved_len(&memory);
        assert_eq!(reserved, bound + 0x8000_0000);

        // Grow, then dirty both the original prefix and the grown tail.
        assert_eq!(memory.grow(Pages(3)).unwrap(), Pages(2));
        assert_eq!(memory.size(), Pages(5));
        unsafe {
            *base = 0xAA;
            *base.add(4 * page) = 0xBB;
        }

        memory.shrink_to_minimum().unwrap();

        // Logical size and the definition seen by compiled code are back at
        // the minimum, the mapping and its base are untouched, and the
        // prefix is zeroed.
        assert_eq!(memory.size(), Pages(2));
        let md = unsafe { md_ptr.as_ref() };
        assert_eq!(md.base, base);
        assert_eq!(md.current_length, 2 * page);
        assert_eq!(reserved_len(&memory), reserved);
        assert_eq!(unsafe { *base }, 0);

        // The grown tail is inaccessible again, and so is everything up to the
        // end of the bound and the guard behind it.
        assert_inaccessible(base, 2 * page);
        assert_inaccessible(base, 4 * page);
        assert_inaccessible(base, bound - page);
        assert_inaccessible(base, bound + 0x8000_0000 - page);

        // Growing again reuses the same mapping and yields zeroed pages.
        assert_eq!(memory.grow(Pages(3)).unwrap(), Pages(2));
        let md = unsafe { md_ptr.as_ref() };
        assert_eq!(md.base, base);
        assert_eq!(md.current_length, 5 * page);
        assert_eq!(reserved_len(&memory), reserved);
        assert_eq!(unsafe { *base.add(4 * page) }, 0);
    }

    #[test]
    fn static_shrink_at_minimum_zeroes_in_place() {
        let ty = MemoryType::new(Pages(1), None, false);
        let memory = LinearMemory::new(&ty, &static_style()).unwrap();
        let md_ptr = memory.vmmemory();
        let base = unsafe { md_ptr.as_ref().base };
        let reserved = reserved_len(&memory);
        unsafe {
            *base.add(100) = 0xAA;
        }

        memory.shrink_to_minimum().unwrap();

        let md = unsafe { md_ptr.as_ref() };
        assert_eq!(memory.size(), Pages(1));
        assert_eq!(md.base, base);
        assert_eq!(reserved_len(&memory), reserved);
        assert_eq!(unsafe { *base.add(100) }, 0);
    }

    #[test]
    fn dynamic_shrink_reallocates_minimum_plus_guard() {
        let page = Pages(1).bytes().0;
        let guard = 0x1_0000;
        let ty = MemoryType::new(Pages(1), None, false);
        let style = MemoryStyle::Dynamic {
            offset_guard_size: guard as u64,
        };
        let memory = LinearMemory::new(&ty, &style).unwrap();
        let md_ptr = memory.vmmemory();

        assert_eq!(memory.grow(Pages(2)).unwrap(), Pages(1));
        unsafe {
            *md_ptr.as_ref().base = 0xAA;
        }

        memory.shrink_to_minimum().unwrap();

        let md = unsafe { md_ptr.as_ref() };
        assert_eq!(memory.size(), Pages(1));
        assert_eq!(md.current_length, page);
        assert_eq!(reserved_len(&memory), page + guard);
        assert_eq!(unsafe { *md.base }, 0);

        // Growing after the shrink still works and keeps the data.
        unsafe {
            *md.base = 0xCC;
        }
        assert_eq!(memory.grow(Pages(1)).unwrap(), Pages(1));
        assert_eq!(unsafe { *md_ptr.as_ref().base }, 0xCC);
    }
}
