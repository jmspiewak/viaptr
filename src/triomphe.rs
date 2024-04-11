use core::{mem::align_of, sync::atomic::AtomicUsize};

use triomphe::{Arc, ThinArc};

use crate::{max, Aligned, CloneInPlace, Leak, NonNull, Pointer};

unsafe impl<T> Pointer for Arc<T> {
    fn into_ptr(value: Self) -> *const () {
        Arc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Leak<Self> {
        Leak::new(unsafe { Arc::from_raw(ptr.cast()) })
    }
}

unsafe impl<T> NonNull for Arc<T> {}

unsafe impl<T> Aligned for Arc<T> {
    const ALIGNMENT: usize = max(align_of::<AtomicUsize>(), align_of::<T>());
}

unsafe impl<T> CloneInPlace for Arc<T> {}


unsafe impl<H, T> Pointer for ThinArc<H, T> {
    fn into_ptr(value: Self) -> *const () {
        ThinArc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Leak<Self> {
        Leak::new(unsafe { ThinArc::from_raw(ptr.cast()) })
    }
}

unsafe impl<H, T> NonNull for ThinArc<H, T> {}

unsafe impl<H, T> Aligned for ThinArc<H, T> {
    const ALIGNMENT: usize = max(
        align_of::<AtomicUsize>(),
        max(align_of::<H>(), align_of::<T>()),
    );
}

unsafe impl<H, T> CloneInPlace for ThinArc<H, T> {}
