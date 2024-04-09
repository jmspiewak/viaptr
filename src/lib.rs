#![feature(ptr_mask)]
#![feature(strict_provenance)]
#![no_std]

use core::{mem, mem::align_of, num::NonZeroUsize, ptr};


pub unsafe trait Pointer {
    fn into_ptr(value: Self) -> *const ();
    unsafe fn from_ptr(ptr: *const ()) -> Self;
}

pub unsafe trait NonNull {}

pub unsafe trait Aligned {
    const ALIGNMENT: usize;

    #[doc(hidden)]
    const VALID_ALIGNMENT: () = assert!(Self::ALIGNMENT.is_power_of_two());
}

pub unsafe trait CloneInPlace: Clone {}

pub unsafe fn clone_in_place<T: Pointer + CloneInPlace>(ptr: *const ()) {
    let value = unsafe { T::from_ptr(ptr) };
    mem::forget(value.clone());
    mem::forget(value);
}


unsafe impl<T> Pointer for *const T {
    fn into_ptr(value: Self) -> *const () {
        value.cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        ptr.cast()
    }
}


unsafe impl<T> Pointer for ptr::NonNull<T> {
    fn into_ptr(value: Self) -> *const () {
        value.as_ptr() as *const ()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        ptr::NonNull::new_unchecked(ptr as *mut T)
    }
}

unsafe impl<T> NonNull for ptr::NonNull<T> {}


unsafe impl<T> Pointer for &'static T {
    fn into_ptr(value: Self) -> *const () {
        ptr::from_ref(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        unsafe { &*ptr.cast() }
    }
}

unsafe impl<T> NonNull for &'static T {}

unsafe impl<T> Aligned for &'static T {
    const ALIGNMENT: usize = align_of::<T>();
}

unsafe impl<T> CloneInPlace for &'static T {}


unsafe impl Pointer for usize {
    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value)
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        ptr.addr()
    }
}

unsafe impl CloneInPlace for usize {}


unsafe impl Pointer for NonZeroUsize {
    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value.into())
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        NonZeroUsize::new_unchecked(ptr.addr())
    }
}

unsafe impl NonNull for NonZeroUsize {}

unsafe impl CloneInPlace for NonZeroUsize {}


unsafe impl Pointer for () {
    fn into_ptr(_: Self) -> *const () {
        ptr::without_provenance(Self::ALIGNMENT)
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        debug_assert!(ptr.addr() == Self::ALIGNMENT);
        ()
    }
}

unsafe impl NonNull for () {}

unsafe impl Aligned for () {
    const ALIGNMENT: usize = 1 << (usize::BITS - 1);
}

unsafe impl CloneInPlace for () {}
