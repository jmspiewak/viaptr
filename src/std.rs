use std::{
    mem::align_of,
    rc,
    rc::Rc,
    sync,
    sync::{atomic::AtomicUsize, Arc},
};

use crate::{max, Aligned, CloneInPlace, MaybeOwned, NonNull, Pointer};


unsafe impl<T> Pointer for Box<T> {
    fn into_ptr(value: Self) -> *const () {
        Box::into_raw(value) as *const ()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Box::from_raw(ptr as *mut T) })
    }
}

unsafe impl<T> NonNull for Box<T> {}

unsafe impl<T> Aligned for Box<T> {
    const ALIGNMENT: usize = align_of::<T>();
}


unsafe impl<T> Pointer for Rc<T> {
    fn into_ptr(value: Self) -> *const () {
        Rc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Rc::from_raw(ptr.cast()) })
    }
}

unsafe impl<T> NonNull for Rc<T> {}

unsafe impl<T> Aligned for Rc<T> {
    const ALIGNMENT: usize = max(align_of::<usize>(), align_of::<T>());
}

unsafe impl<T> CloneInPlace for Rc<T> {}


unsafe impl<T> Pointer for rc::Weak<T> {
    fn into_ptr(value: Self) -> *const () {
        rc::Weak::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { rc::Weak::from_raw(ptr.cast()) })
    }
}

unsafe impl<T> CloneInPlace for rc::Weak<T> {}


unsafe impl<T> Pointer for Arc<T> {
    fn into_ptr(value: Self) -> *const () {
        Arc::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { Arc::from_raw(ptr.cast()) })
    }
}

unsafe impl<T> NonNull for Arc<T> {}

unsafe impl<T> Aligned for Arc<T> {
    const ALIGNMENT: usize = max(align_of::<AtomicUsize>(), align_of::<T>());
}

unsafe impl<T> CloneInPlace for Arc<T> {}


unsafe impl<T> Pointer for sync::Weak<T> {
    fn into_ptr(value: Self) -> *const () {
        sync::Weak::into_raw(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { sync::Weak::from_raw(ptr.cast()) })
    }
}

unsafe impl<T> CloneInPlace for sync::Weak<T> {}
