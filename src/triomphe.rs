use core::{mem::align_of, sync::atomic::AtomicUsize};

use triomphe::{Arc, ThinArc};

use crate::{max, MaybeOwned, Pointer};


unsafe impl<T> Pointer for Arc<T> {
    const NON_NULL: bool = true;
    const ALIGNMENT: usize = max(align_of::<AtomicUsize>(), align_of::<T>());
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        Arc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Arc::from_raw(ptr.cast()) })
    }
}


unsafe impl<H, T> Pointer for ThinArc<H, T> {
    const NON_NULL: bool = true;
    const CLONE_IN_PLACE: bool = true;

    const ALIGNMENT: usize = max(
        align_of::<AtomicUsize>(),
        max(align_of::<H>(), align_of::<T>()),
    );

    fn into_ptr(value: Self) -> *const () {
        ThinArc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { ThinArc::from_raw(ptr.cast()) })
    }
}
