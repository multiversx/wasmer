#[cfg(unix)]
use crate::sys::unix::{Memory, Protect};

#[cfg(windows)]
use crate::sys::windows::{Memory, Protect};

use rkyv::{
    Archive, 
    Fallible,
    Serialize as RkyvSerialize,
    Deserialize as RkyvDeserialize,
    ser::{Serializer, ScratchSpace},
    with::{ArchiveWith, SerializeWith, DeserializeWith},
};

/// A serializable wrapper for Memory.
pub struct ArchivableMemory;

/// The archived contents of a wrapped Memory.
#[cfg(unix)]
#[derive(Debug, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct ArchivedMemory {
    contents: Vec<u8>,
    protection: Protect,
}

impl ArchivedMemory {
    /// Construct an ArchivedMemory from a Memory.
    pub unsafe fn from_memory(memory: &Memory) -> Self {
        ArchivedMemory {
            contents: memory.as_slice().to_vec(),
            protection: memory.protection(),
        }
    }
}

impl ArchiveWith<Memory> for ArchivableMemory {
    type Archived = <ArchivedMemory as Archive>::Archived;
    type Resolver = <ArchivedMemory as Archive>::Resolver;

    unsafe fn resolve_with(memory: &Memory, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        let archived_memory = ArchivedMemory::from_memory(memory);
        archived_memory.resolve(pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<Memory, S> for ArchivableMemory 
where
    Memory: RkyvSerialize<S>,
    S: Serializer + ScratchSpace
{
    fn serialize_with(memory: &Memory, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        unsafe {
            let archived_memory = ArchivedMemory::from_memory(memory);
            archived_memory.serialize(serializer)
        }
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedMemory, Memory, D> for ArchivableMemory
where
    ArchivedMemory: RkyvDeserialize<Memory, D>,
{
    fn deserialize_with(archived_memory: &ArchivedMemory, _: &mut D) -> Result<Memory, D::Error> {
        let original_protection = archived_memory.protection;
        let bytes = archived_memory.contents.as_slice();

        let mut memory = Memory::with_size_protect(bytes.len(), Protect::ReadWrite)
            .expect("Could not create a memory");

        unsafe {
            memory.as_slice_mut().copy_from_slice(&*bytes);

            if memory.protection() != original_protection {
                memory
                    .protect(.., original_protection)
                    .expect("Could not protect memory as its original protection");
            }
        }

        Ok(memory)
    }
}

#[cfg(test)]
mod tests {
    use crate::sys::unix::*;

    #[test]
    fn test_new_memory() {
        let bytes = b"abcdefghijkl";
        let mut memory = Memory::with_size_protect(1000, Protect::ReadWrite)
            .expect("Could not create memory");

        unsafe {
            memory.as_slice_mut().copy_from_slice(&bytes[..]);
            assert_eq!(memory.as_slice(), &bytes[..]);
        }
    }
}
