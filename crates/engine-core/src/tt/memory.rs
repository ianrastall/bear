//! The memory behind the transposition table: one zeroed block aligned to 2 MiB, so
//! that the operating system can back it with large pages and each memory access
//! needs fewer address translations.
//!
//! On Windows the block comes from large pages when the account holds the "Lock pages
//! in memory" right; on Linux the kernel is asked to use transparent huge pages for
//! it. Anywhere else, or when that fails, it is ordinary memory with the same
//! alignment. This is unsafe site 5 in the architecture document.

#![allow(unsafe_code)]

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ptr::NonNull;

/// The alignment of the block, the size of an x86-64 large page.
const ALIGN: usize = 2 << 20;

/// Where a block came from, which decides how it is freed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Pages {
    /// Windows large pages, freed with `VirtualFree`.
    Large,
    /// The global allocator, freed with the same layout.
    Normal,
}

/// A zeroed, 2 MiB-aligned array of `len` values of `T`.
pub(super) struct ZeroedBlock<T> {
    ptr: NonNull<T>,
    len: usize,
    bytes: usize,
    pages: Pages,
}

// SAFETY: the block owns its memory outright, like a `Box<[T]>`, so sending or sharing
// it is exactly as safe as sending or sharing the values in it.
unsafe impl<T: Send> Send for ZeroedBlock<T> {}
// SAFETY: as above; shared access only ever hands out `&[T]`.
unsafe impl<T: Sync> Sync for ZeroedBlock<T> {}

impl<T> ZeroedBlock<T> {
    /// An empty block, which owns no memory.
    pub(super) fn empty() -> ZeroedBlock<T> {
        ZeroedBlock {
            ptr: NonNull::dangling(),
            len: 0,
            bytes: 0,
            pages: Pages::Normal,
        }
    }

    /// `len` values of `T`, or `None` if the memory is not available.
    ///
    /// # Safety
    ///
    /// Memory filled with zero bytes must be a valid `T`.
    pub(super) unsafe fn new(len: usize) -> Option<ZeroedBlock<T>> {
        assert!(align_of::<T>() <= ALIGN && size_of::<T>() > 0);
        let bytes = len.checked_mul(size_of::<T>())?;
        if bytes == 0 {
            return Some(ZeroedBlock::empty());
        }
        if let Some((ptr, bytes)) = large_pages(bytes) {
            return Some(ZeroedBlock {
                ptr: ptr.cast(),
                len,
                bytes,
                pages: Pages::Large,
            });
        }
        let layout = Layout::from_size_align(bytes, ALIGN).ok()?;
        // SAFETY: the layout has a non-zero size, checked above.
        let ptr = NonNull::new(unsafe { alloc_zeroed(layout) })?;
        advise_huge_pages(ptr, bytes);
        Some(ZeroedBlock {
            ptr: ptr.cast(),
            len,
            bytes,
            pages: Pages::Normal,
        })
    }

    pub(super) fn pages(&self) -> Pages {
        self.pages
    }
}

impl<T> std::ops::Deref for ZeroedBlock<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // SAFETY: the block holds `len` zero-initialised values of `T`, which `new`'s
        // caller vouched are valid, at a well-aligned address that lives as long as
        // the block; an empty block has a dangling but aligned pointer and length 0.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> Drop for ZeroedBlock<T> {
    fn drop(&mut self) {
        if self.bytes == 0 {
            return;
        }
        match self.pages {
            Pages::Large => free_large_pages(self.ptr.cast()),
            Pages::Normal => {
                let layout = Layout::from_size_align(self.bytes, ALIGN)
                    .expect("the layout was valid when the block was allocated");
                // SAFETY: the pointer came from `alloc_zeroed` with this same layout.
                unsafe { dealloc(self.ptr.as_ptr().cast(), layout) }
            }
        }
    }
}

#[cfg(all(target_os = "linux", not(miri)))]
fn advise_huge_pages(ptr: NonNull<u8>, bytes: usize) {
    const MADV_HUGEPAGE: i32 = 14;
    unsafe extern "C" {
        fn madvise(addr: *mut std::ffi::c_void, len: usize, advice: i32) -> i32;
    }
    // SAFETY: the range is one block that this process has just allocated. The advice
    // only changes how the kernel backs it; a refusal is harmless and ignored.
    unsafe {
        madvise(ptr.as_ptr().cast(), bytes, MADV_HUGEPAGE);
    }
}

#[cfg(not(all(target_os = "linux", not(miri))))]
fn advise_huge_pages(_ptr: NonNull<u8>, _bytes: usize) {}

#[cfg(all(windows, not(miri)))]
mod windows {
    use std::ffi::c_void;
    use std::ptr::{NonNull, null, null_mut};
    use std::sync::OnceLock;

    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const MEM_LARGE_PAGES: u32 = 0x2000_0000;
    const MEM_RELEASE: u32 = 0x8000;
    const PAGE_READWRITE: u32 = 0x04;
    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x20;
    const TOKEN_QUERY: u32 = 0x08;
    const SE_PRIVILEGE_ENABLED: u32 = 0x02;
    const ERROR_SUCCESS: u32 = 0;

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct Luid {
        low: u32,
        high: i32,
    }

    #[repr(C)]
    struct TokenPrivileges {
        count: u32,
        luid: Luid,
        attributes: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
        fn VirtualFree(addr: *mut c_void, size: usize, kind: u32) -> i32;
        fn GetLargePageMinimum() -> usize;
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process: *mut c_void, access: u32, token: *mut *mut c_void) -> i32;
        fn LookupPrivilegeValueW(system: *const u16, name: *const u16, luid: *mut Luid) -> i32;
        fn AdjustTokenPrivileges(
            token: *mut c_void,
            disable_all: i32,
            new: *const TokenPrivileges,
            length: u32,
            previous: *mut TokenPrivileges,
            returned: *mut u32,
        ) -> i32;
    }

    /// Enables the "Lock pages in memory" privilege for this process, once. Large
    /// pages cannot be allocated without it, and accounts do not hold it by default.
    fn privilege_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let name: Vec<u16> = "SeLockMemoryPrivilege\0".encode_utf16().collect();
            let mut token = null_mut();
            // SAFETY: every pointer passed is to a live local of the declared type,
            // the name is NUL-terminated, and the token handle is closed on every
            // path once it has been opened.
            unsafe {
                if OpenProcessToken(
                    GetCurrentProcess(),
                    TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                    &mut token,
                ) == 0
                {
                    return false;
                }
                let mut luid = Luid::default();
                let mut enabled = false;
                if LookupPrivilegeValueW(null(), name.as_ptr(), &mut luid) != 0 {
                    let wanted = TokenPrivileges {
                        count: 1,
                        luid,
                        attributes: SE_PRIVILEGE_ENABLED,
                    };
                    // The call succeeds even when the account lacks the right, so the
                    // last error says whether it was actually granted.
                    enabled = AdjustTokenPrivileges(token, 0, &wanted, 0, null_mut(), null_mut())
                        != 0
                        && GetLastError() == ERROR_SUCCESS;
                }
                CloseHandle(token);
                enabled
            }
        })
    }

    /// A zeroed block of at least `bytes` from large pages, with its actual size.
    pub(super) fn large_pages(bytes: usize) -> Option<(NonNull<u8>, usize)> {
        if !privilege_enabled() {
            return None;
        }
        // SAFETY: no arguments; the call only reports a size.
        let page = unsafe { GetLargePageMinimum() };
        if page == 0 {
            return None;
        }
        let rounded = bytes.checked_next_multiple_of(page)?;
        // SAFETY: a fresh reservation at an address of the system's choosing; the
        // system zeroes committed pages, and a failure returns null.
        let ptr = unsafe {
            VirtualAlloc(
                null_mut(),
                rounded,
                MEM_RESERVE | MEM_COMMIT | MEM_LARGE_PAGES,
                PAGE_READWRITE,
            )
        };
        NonNull::new(ptr.cast()).map(|ptr| (ptr, rounded))
    }

    pub(super) fn free_large_pages(ptr: NonNull<u8>) {
        // SAFETY: the pointer is the base of a block from `VirtualAlloc`, released
        // whole, as `MEM_RELEASE` requires, and never used again.
        unsafe {
            VirtualFree(ptr.as_ptr().cast(), 0, MEM_RELEASE);
        }
    }
}

#[cfg(all(windows, not(miri)))]
use windows::{free_large_pages, large_pages};

#[cfg(not(all(windows, not(miri))))]
fn large_pages(_bytes: usize) -> Option<(NonNull<u8>, usize)> {
    None
}

#[cfg(not(all(windows, not(miri))))]
fn free_large_pages(_ptr: NonNull<u8>) {
    unreachable!("large pages are never allocated on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_zeroed_and_aligned() {
        // SAFETY: zero is a valid `u64`.
        let block = unsafe { ZeroedBlock::<u64>::new(3 << 15) }.unwrap();
        assert_eq!(block.len(), 3 << 15);
        assert_eq!(block.as_ptr() as usize % ALIGN, 0);
        assert!(block.iter().all(|&x| x == 0));
        let empty = ZeroedBlock::<u64>::empty();
        assert!(empty.is_empty());
    }
}
