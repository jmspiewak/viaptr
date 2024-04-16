use core::{mem::align_of, sync::atomic::AtomicUsize};
use std::{rc, rc::Rc, sync, sync::Arc};

use crate::{max, MaybeOwned, Pointer};


unsafe impl<T> Pointer for Box<T> {
    const NON_NULL: bool = true;
    const ALIGNMENT: usize = align_of::<T>();

    fn into_ptr(value: Self) -> *const () {
        Box::into_raw(value) as *const ()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Box::from_raw(ptr as *mut T) })
    }
}


unsafe impl<T> Pointer for Rc<T> {
    const NON_NULL: bool = true;
    const ALIGNMENT: usize = max(align_of::<usize>(), align_of::<T>());
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        Rc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Rc::from_raw(ptr.cast()) })
    }
}


unsafe impl<T> Pointer for rc::Weak<T> {
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        rc::Weak::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { rc::Weak::from_raw(ptr.cast()) })
    }
}


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


unsafe impl<T> Pointer for sync::Weak<T> {
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        sync::Weak::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { sync::Weak::from_raw(ptr.cast()) })
    }
}
