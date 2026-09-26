//! The reference C implementation of yespower (vendored in `yespower/`), behind the `c` feature:
//! for checking the Rust port against it and for comparing their speed.

use std::ffi::c_void;

use crate::Params;

/// yespower 1.0, as opposed to the older yescrypt 0.5 variant.
const YESPOWER_1_0: u32 = 10;

#[repr(C)]
struct Region {
    base: *mut c_void,
    aligned: *mut c_void,
    base_size: usize,
    aligned_size: usize,
}

#[repr(C)]
struct RawParams {
    version: u32,
    n: u32,
    r: u32,
    pers: *const u8,
    perslen: usize,
}

extern "C" {
    fn yespower_init_local(local: *mut Region) -> i32;
    fn yespower_free_local(local: *mut Region) -> i32;
    fn yespower(
        local: *mut Region,
        src: *const u8,
        srclen: usize,
        params: *const RawParams,
        dst: *mut [u8; 32],
    ) -> i32;
}

/// A yespower hasher with its working memory, kept between hashes. One per thread.
pub struct Hasher {
    local: Region,
    params: Params,
}

// The working memory belongs to the hasher alone; it may move between threads but not be shared.
unsafe impl Send for Hasher {}

impl Hasher {
    pub fn new(params: Params) -> Self {
        let mut local = Region {
            base: std::ptr::null_mut(),
            aligned: std::ptr::null_mut(),
            base_size: 0,
            aligned_size: 0,
        };
        // SAFETY: `local` is a valid region for the library to initialize.
        let rc = unsafe { yespower_init_local(&mut local) };
        assert_eq!(rc, 0, "yespower_init_local failed");
        Self { local, params }
    }

    /// yespower 1.0 of `input` with this hasher's parameters.
    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        let raw = RawParams {
            version: YESPOWER_1_0,
            n: self.params.n,
            r: self.params.r,
            pers: self.params.pers.as_ptr(),
            perslen: self.params.pers.len(),
        };
        let mut out = [0u8; 32];
        // SAFETY: the pointers are valid for the lengths given; `local` was initialized in `new`.
        let rc = unsafe { yespower(&mut self.local, input.as_ptr(), input.len(), &raw, &mut out) };
        assert_eq!(rc, 0, "yespower failed (out of memory?)");
        out
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // SAFETY: `local` was initialized in `new` and is freed once.
        unsafe { yespower_free_local(&mut self.local) };
    }
}
