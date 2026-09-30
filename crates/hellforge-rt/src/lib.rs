//! The handful of C runtime symbols LLVM expects (`memcpy` and friends),
//! written in Rust so binaries need no libc. `no_builtins` stops LLVM from
//! "optimising" these loops back into calls to themselves.

#![no_std]
#![no_builtins]
#![allow(clippy::missing_safety_doc)]

const W: usize = core::mem::size_of::<usize>();

#[inline(always)]
unsafe fn memcpy_impl(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    unsafe {
        if (dest as usize | src as usize) & (W - 1) == 0 {
            while i + W <= n {
                *(dest.add(i) as *mut usize) = *(src.add(i) as *const usize);
                i += W;
            }
        }
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    }
    dest
}

#[inline(always)]
unsafe fn memmove_impl(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        if (dest as usize) <= (src as usize) || (dest as usize) >= (src as usize + n) {
            return memcpy_impl(dest, src, n);
        }
        // Overlapping with dest after src: copy backwards.
        let mut i = n;
        if (dest as usize | src as usize | n) & (W - 1) == 0 {
            while i >= W {
                i -= W;
                *(dest.add(i) as *mut usize) = *(src.add(i) as *const usize);
            }
        }
        while i > 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

#[inline(always)]
unsafe fn memset_impl(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let b = c as u8;
    let mut i = 0;
    unsafe {
        if dest as usize & (W - 1) == 0 {
            let word = usize::from_ne_bytes([b; W]);
            while i + W <= n {
                *(dest.add(i) as *mut usize) = word;
                i += W;
            }
        }
        while i < n {
            *dest.add(i) = b;
            i += 1;
        }
    }
    dest
}

#[inline(always)]
unsafe fn memcmp_impl(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}

#[inline(always)]
unsafe fn bcmp_impl(a: *const u8, b: *const u8, n: usize) -> i32 {
    unsafe { memcmp_impl(a, b, n) }
}

#[inline(always)]
unsafe fn strlen_impl(s: *const u8) -> usize {
    let mut n = 0;
    unsafe {
        while *s.add(n) != 0 {
            n += 1;
        }
    }
    n
}

// Apple targets: LLVM lowers some fills to these libSystem-specific calls.
#[cfg(target_vendor = "apple")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bzero(s: *mut u8, n: usize) {
    unsafe {
        memset_impl(s, 0, n);
    }
}

#[cfg(target_vendor = "apple")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __bzero(s: *mut u8, n: usize) {
    unsafe {
        memset_impl(s, 0, n);
    }
}

#[cfg(target_vendor = "apple")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset_pattern16(b: *mut u8, pattern: *const u8, len: usize) {
    for i in 0..len {
        unsafe {
            *b.add(i) = *pattern.add(i % 16);
        }
    }
}

// The exported C symbols, with C's types. `no_builtins` above keeps LLVM from
// turning the loops in the helpers back into calls to these.
use core::ffi::{c_char, c_int, c_void};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe { memcpy_impl(dest.cast(), src.cast(), n).cast() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe { memmove_impl(dest.cast(), src.cast(), n).cast() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    unsafe { memset_impl(dest.cast(), c, n).cast() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    unsafe { memcmp_impl(a.cast(), b.cast(), n) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    unsafe { bcmp_impl(a.cast(), b.cast(), n) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    unsafe { strlen_impl(s.cast()) }
}
