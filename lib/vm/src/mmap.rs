// This file contains code from external sources.
// Attributions: https://github.com/wasmerio/wasmer/blob/master/ATTRIBUTIONS.md

//! Low-level abstraction for allocating and managing zero-filled pages
//! of memory.

use loupe::{MemoryUsage, MemoryUsageTracker};
use more_asserts::assert_le;
use more_asserts::assert_lt;
use std::io;
use std::ptr;
use std::slice;

/// Round `size` up to the nearest multiple of `page_size`.
fn round_up_to_page_size(size: usize, page_size: usize) -> usize {
    (size + (page_size - 1)) & !(page_size - 1)
}

/// A simple struct consisting of a page-aligned pointer to page-aligned
/// and initially-zeroed memory and a length.
#[derive(Debug)]
pub struct Mmap {
    // Note that this is stored as a `usize` instead of a `*const` or `*mut`
    // pointer to allow this structure to be natively `Send` and `Sync` without
    // `unsafe impl`. This type is sendable across threads and shareable since
    // the coordination all happens at the OS layer.
    ptr: usize,
    len: usize,
}

impl Mmap {
    /// Construct a new empty instance of `Mmap`.
    pub fn new() -> Self {
        // Rust's slices require non-null pointers, even when empty. `Vec`
        // contains code to create a non-null dangling pointer value when
        // constructed empty, so we reuse that here.
        let empty = Vec::<u8>::new();
        Self {
            ptr: empty.as_ptr() as usize,
            len: 0,
        }
    }

    /// Create a new `Mmap` pointing to at least `size` bytes of page-aligned accessible memory.
    pub fn with_at_least(size: usize) -> Result<Self, String> {
        let page_size = region::page::size();
        let rounded_size = round_up_to_page_size(size, page_size);
        Self::accessible_reserved(rounded_size, rounded_size)
    }

    /// Create a new `Mmap` pointing to `accessible_size` bytes of page-aligned accessible memory,
    /// within a reserved mapping of `mapping_size` bytes. `accessible_size` and `mapping_size`
    /// must be native page-size multiples.
    #[cfg(not(target_os = "windows"))]
    pub fn accessible_reserved(
        accessible_size: usize,
        mapping_size: usize,
    ) -> Result<Self, String> {
        let page_size = region::page::size();
        assert_le!(accessible_size, mapping_size);
        assert_eq!(mapping_size & (page_size - 1), 0);
        assert_eq!(accessible_size & (page_size - 1), 0);

        // Mmap may return EINVAL if the size is zero, so just
        // special-case that.
        if mapping_size == 0 {
            return Ok(Self::new());
        }

        Ok(if accessible_size == mapping_size {
            // Allocate a single read-write region at once.
            let ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    mapping_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                )
            };
            if ptr as isize == -1_isize {
                return Err(io::Error::last_os_error().to_string());
            }

            Self {
                ptr: ptr as usize,
                len: mapping_size,
            }
        } else {
            // Reserve the mapping size.
            let ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    mapping_size,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                )
            };
            if ptr as isize == -1_isize {
                return Err(io::Error::last_os_error().to_string());
            }

            let mut result = Self {
                ptr: ptr as usize,
                len: mapping_size,
            };

            if accessible_size != 0 {
                // Commit the accessible size.
                result.make_accessible(0, accessible_size)?;
            }

            result
        })
    }

    /// Create a new `Mmap` pointing to `accessible_size` bytes of page-aligned accessible memory,
    /// within a reserved mapping of `mapping_size` bytes. `accessible_size` and `mapping_size`
    /// must be native page-size multiples.
    #[cfg(target_os = "windows")]
    pub fn accessible_reserved(
        accessible_size: usize,
        mapping_size: usize,
    ) -> Result<Self, String> {
        use winapi::um::memoryapi::VirtualAlloc;
        use winapi::um::winnt::{MEM_COMMIT, MEM_RESERVE, PAGE_NOACCESS, PAGE_READWRITE};

        let page_size = region::page::size();
        assert_le!(accessible_size, mapping_size);
        assert_eq!(mapping_size & (page_size - 1), 0);
        assert_eq!(accessible_size & (page_size - 1), 0);

        // VirtualAlloc may return ERROR_INVALID_PARAMETER if the size is zero,
        // so just special-case that.
        if mapping_size == 0 {
            return Ok(Self::new());
        }

        Ok(if accessible_size == mapping_size {
            // Allocate a single read-write region at once.
            let ptr = unsafe {
                VirtualAlloc(
                    ptr::null_mut(),
                    mapping_size,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            };
            if ptr.is_null() {
                return Err(io::Error::last_os_error().to_string());
            }

            Self {
                ptr: ptr as usize,
                len: mapping_size,
            }
        } else {
            // Reserve the mapping size.
            let ptr =
                unsafe { VirtualAlloc(ptr::null_mut(), mapping_size, MEM_RESERVE, PAGE_NOACCESS) };
            if ptr.is_null() {
                return Err(io::Error::last_os_error().to_string());
            }

            let mut result = Self {
                ptr: ptr as usize,
                len: mapping_size,
            };

            if accessible_size != 0 {
                // Commit the accessible size.
                result.make_accessible(0, accessible_size)?;
            }

            result
        })
    }

    /// Make the memory starting at `start` and extending for `len` bytes accessible.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(not(target_os = "windows"))]
    pub fn make_accessible(&mut self, start: usize, len: usize) -> Result<(), String> {
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_lt!(len, self.len);
        assert_lt!(start, self.len - len);

        // Commit the accessible size.
        let ptr = self.ptr as *const u8;
        unsafe { region::protect(ptr.add(start), len, region::Protection::READ_WRITE) }
            .map_err(|e| e.to_string())
    }

    /// Make the memory starting at `start` and extending for `len` bytes accessible.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(target_os = "windows")]
    pub fn make_accessible(&mut self, start: usize, len: usize) -> Result<(), String> {
        use winapi::ctypes::c_void;
        use winapi::um::memoryapi::VirtualAlloc;
        use winapi::um::winnt::{MEM_COMMIT, PAGE_READWRITE};
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_lt!(len, self.len);
        assert_lt!(start, self.len - len);

        // Commit the accessible size.
        let ptr = self.ptr as *const u8;
        if unsafe {
            VirtualAlloc(
                ptr.add(start) as *mut c_void,
                len,
                MEM_COMMIT,
                PAGE_READWRITE,
            )
        }
        .is_null()
        {
            return Err(io::Error::last_os_error().to_string());
        }

        Ok(())
    }

    /// Return the memory starting at `start` and extending for `len` bytes to
    /// the reserved-but-inaccessible state it had before `make_accessible` was
    /// called on it. The pages become inaccessible and their contents are
    /// discarded, so a later `make_accessible` on the same range yields
    /// zero-filled pages again. The mapping itself stays in place: `as_ptr()`
    /// and `len()` are unchanged and the range remains reserved for this `Mmap`.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(not(target_os = "windows"))]
    pub fn decommit(&mut self, start: usize, len: usize) -> Result<(), String> {
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_le!(len, self.len);
        assert_le!(start, self.len - len);

        if len == 0 {
            return Ok(());
        }

        // Map fresh anonymous `PROT_NONE` pages over the range. `MAP_FIXED`
        // atomically replaces the existing pages, dropping their contents and
        // letting the OS reclaim the physical frames. Unlike
        // `madvise(MADV_DONTNEED)` this behaves the same on Linux and macOS;
        // on macOS `MADV_DONTNEED` only deactivates pages without discarding
        // their contents.
        let ptr = unsafe {
            libc::mmap(
                (self.ptr + start) as *mut libc::c_void,
                len,
                libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_FIXED,
                -1,
                0,
            )
        };
        if ptr as isize == -1_isize {
            return Err(io::Error::last_os_error().to_string());
        }
        debug_assert_eq!(ptr as usize, self.ptr + start);

        Ok(())
    }

    /// Return the memory starting at `start` and extending for `len` bytes to
    /// the reserved-but-inaccessible state it had before `make_accessible` was
    /// called on it. The pages become inaccessible and their contents are
    /// discarded, so a later `make_accessible` on the same range yields
    /// zero-filled pages again. The mapping itself stays in place: `as_ptr()`
    /// and `len()` are unchanged and the range remains reserved for this `Mmap`.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(target_os = "windows")]
    pub fn decommit(&mut self, start: usize, len: usize) -> Result<(), String> {
        use winapi::ctypes::c_void;
        use winapi::um::memoryapi::VirtualFree;
        use winapi::um::winnt::MEM_DECOMMIT;
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_le!(len, self.len);
        assert_le!(start, self.len - len);

        if len == 0 {
            return Ok(());
        }

        // Decommit the pages but keep them reserved. Committing them again
        // with `make_accessible` yields zero-filled pages.
        let ptr = self.ptr as *const u8;
        if unsafe { VirtualFree(ptr.add(start) as *mut c_void, len, MEM_DECOMMIT) } == 0 {
            return Err(io::Error::last_os_error().to_string());
        }

        Ok(())
    }

    /// Return the allocated memory as a slice of u8.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    /// Return the allocated memory as a mutable slice of u8.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.len) }
    }

    /// Return the allocated memory as a pointer to u8.
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    /// Return the allocated memory as a mutable pointer to u8.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr as *mut u8
    }

    /// Return the length of the allocated memory.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether any memory has been allocated.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for Mmap {
    #[cfg(not(target_os = "windows"))]
    fn drop(&mut self) {
        if self.len != 0 {
            let r = unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
            assert_eq!(r, 0, "munmap failed: {}", io::Error::last_os_error());
        }
    }

    #[cfg(target_os = "windows")]
    fn drop(&mut self) {
        if self.len != 0 {
            use winapi::ctypes::c_void;
            use winapi::um::memoryapi::VirtualFree;
            use winapi::um::winnt::MEM_RELEASE;
            let r = unsafe { VirtualFree(self.ptr as *mut c_void, 0, MEM_RELEASE) };
            assert_ne!(r, 0);
        }
    }
}

impl MemoryUsage for Mmap {
    fn size_of_val(&self, tracker: &mut dyn MemoryUsageTracker) -> usize {
        tracker.track(self.as_ptr() as *const ());

        self.len()
    }
}

fn _assert() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<Mmap>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_up_to_page_size() {
        assert_eq!(round_up_to_page_size(0, 4096), 0);
        assert_eq!(round_up_to_page_size(1, 4096), 4096);
        assert_eq!(round_up_to_page_size(4096, 4096), 4096);
        assert_eq!(round_up_to_page_size(4097, 4096), 8192);
    }

    #[test]
    fn decommit_keeps_mapping_and_zeroes_on_recommit() {
        let page = region::page::size();
        let mut mmap = Mmap::accessible_reserved(2 * page, 4 * page).unwrap();
        let ptr = mmap.as_mut_ptr();
        unsafe {
            *ptr = 0xAA;
            *ptr.add(page) = 0xBB;
        }

        mmap.decommit(page, page).unwrap();

        // Same mapping, same length, first page untouched.
        assert_eq!(mmap.as_mut_ptr(), ptr);
        assert_eq!(mmap.len(), 4 * page);
        assert_eq!(unsafe { *ptr }, 0xAA);
        // The decommitted page is inaccessible, like the never-committed ones.
        let decommitted = region::query(unsafe { ptr.add(page) }).unwrap();
        assert_eq!(decommitted.protection(), region::Protection::NONE);
        let reserved = region::query(unsafe { ptr.add(2 * page) }).unwrap();
        assert_eq!(reserved.protection(), region::Protection::NONE);

        // Committing the page again yields zeroes, not the old contents.
        mmap.make_accessible(page, page).unwrap();
        assert_eq!(mmap.as_mut_ptr(), ptr);
        assert_eq!(unsafe { *ptr.add(page) }, 0);
    }
}
