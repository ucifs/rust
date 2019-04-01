//! This module consists of functions needed by the SGX port of libunwind.
//! Since SGX does not have any libc it must link against some other
//! implementation of the things it needs access to.
//!
//! This causes a circular dependency between `libunwind.a` and `libstd.rlib`.
//! So this code must be placed somewhere that allows it to be present
//! when libunwind happens to be linked.

#[cfg(not(test))]
use crate::{
    alloc::{self, Layout},
    lock_api::RawRwLock as _,
    slice, str,
    sync::atomic::Ordering,
};
use crate::{parking_lot::RawRwLock, sync::atomic::AtomicBool};

#[cfg(not(test))]
const EINVAL: i32 = 22;

#[repr(C)]
pub struct RwLock {
    lock: RawRwLock,
    is_write_locked: AtomicBool,
}

// used by libunwind port
#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_rdlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    (*p).lock.lock_shared();
    return 0;
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_wrlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    (*p).lock.lock_exclusive();
    (*p).is_write_locked.store(true, Ordering::Relaxed);
    return 0;
}
#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_unlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    if (*p)
        .is_write_locked
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        (*p).lock.unlock_exclusive()
    } else {
        (*p).lock.unlock_shared();
    }
    return 0;
}

// the following functions are also used by the libunwind port. They're
// included here to make sure parallel codegen and LTO don't mess things up.
#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_print_err(m: *mut u8, s: i32) {
    if s < 0 {
        return;
    }
    let buf = slice::from_raw_parts(m as *const u8, s as _);
    if let Ok(s) = str::from_utf8(&buf[..buf.iter().position(|&b| b == 0).unwrap_or(buf.len())]) {
        eprint!("{}", s);
    }
}

#[cfg(not(test))]
#[no_mangle]
// NB. used by both libunwind and libpanic_abort
pub unsafe extern "C" fn __rust_abort() {
    crate::sys::abort_internal();
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_c_alloc(size: usize, align: usize) -> *mut u8 {
    alloc::alloc(Layout::from_size_align_unchecked(size, align))
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_c_dealloc(ptr: *mut u8, size: usize, align: usize) {
    alloc::dealloc(ptr, Layout::from_size_align_unchecked(size, align))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{self, MaybeUninit};
    use core::array::FixedSizeArray;

    // Verify that the bytes of an initialized RwLock are the same as in
    // libunwind. If they change, `src/UnwindRustSgx.h` in libunwind needs to
    // be changed too.
    #[test]
    fn test_c_rwlock_initializer() {
        /// The value of a newly initialized `RwLock`. Which happens to be
        /// `RawRwLock::INIT` (a zeroed `usize`), a false boolean (zero)
        /// and then padding.
        const RWLOCK_INIT: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        #[inline(never)]
        fn zero_stack() {
            test::black_box(MaybeUninit::<[RwLock; 16]>::zeroed());
        }

        #[inline(never)]
        unsafe fn rwlock_new(init: &mut MaybeUninit<RwLock>) {
            use crate::lock_api::RawRwLock as _;
            init.write(RwLock {
                lock: RawRwLock::INIT,
                is_write_locked: AtomicBool::new(false),
            });
        }

        unsafe {
            // try hard to make sure that the padding/unused bytes in RwLock
            // get initialized as 0. If the assertion below fails, that might
            // just be an issue with the test code and not with the value of
            // RWLOCK_INIT.
            zero_stack();
            let mut init = MaybeUninit::<RwLock>::zeroed();
            rwlock_new(&mut init);
            assert_eq!(
                mem::transmute::<_, [u8; 16]>(init.assume_init()).as_slice(),
                RWLOCK_INIT
            )
        };
    }

    #[test]
    fn test_rwlock_memory_layout() {
        assert_eq!(mem::size_of::<RwLock>(), mem::size_of::<usize>() * 2);
        assert_eq!(mem::align_of::<RwLock>(), mem::align_of::<usize>());
    }

    #[test]
    fn test_sgx_on_64bit() {
        #[cfg(target_pointer_width = "32")]
        panic!("The RwLock implementation for SGX only works on 64 bit architectures for now");
    }
}
