#![feature(ptr_mask)]
#![feature(strict_provenance)]
#![feature(doc_cfg)]
#![cfg_attr(not(any(feature = "std", doc)), no_std)]

use core::{
    borrow::Borrow,
    mem,
    mem::align_of,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr,
};

#[cfg(any(feature = "std", doc))]
#[doc(cfg(feature = "std"))]
mod std;

#[cfg(any(feature = "triomphe", doc))]
#[doc(cfg(feature = "triomphe"))]
mod triomphe;


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


unsafe impl<T: Pointer + Aligned> Pointer for Option<T> {
    fn into_ptr(value: Self) -> *const () {
        match value {
            Some(x) => T::into_ptr(x),
            None => ptr::without_provenance(Self::ALIGNMENT),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        let tag = ptr.addr() & Self::ALIGNMENT;
        let ptr = ptr.mask(!((Self::ALIGNMENT << 1) - 1));

        unsafe {
            match tag {
                0 => Some(T::from_ptr(ptr)),
                _ => None,
            }
        }
    }
}

unsafe impl<T: NonNull> NonNull for Option<T> {}

unsafe impl<T: Aligned> Aligned for Option<T> {
    const ALIGNMENT: usize = T::ALIGNMENT >> 1;
}

unsafe impl<T: CloneInPlace> CloneInPlace for Option<T> {}


unsafe impl<T: Pointer + Aligned, E: Pointer + Aligned> Pointer for Result<T, E> {
    fn into_ptr(value: Self) -> *const () {
        let (ptr, tag) = match value {
            Ok(x) => (T::into_ptr(x), 0),
            Err(x) => (E::into_ptr(x), Self::ALIGNMENT),
        };

        ptr.map_addr(|a| a | tag)
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        let tag = ptr.addr() & Self::ALIGNMENT;
        let ptr = ptr.mask(!((Self::ALIGNMENT << 1) - 1));

        unsafe {
            match tag {
                0 => Ok(T::from_ptr(ptr)),
                _ => Err(E::from_ptr(ptr)),
            }
        }
    }
}

unsafe impl<T: NonNull, E> NonNull for Result<T, E> {}

unsafe impl<T: Aligned, E: Aligned> Aligned for Result<T, E> {
    const ALIGNMENT: usize = min(T::ALIGNMENT, E::ALIGNMENT) >> 1;
}

unsafe impl<T: CloneInPlace, E: CloneInPlace> CloneInPlace for Result<T, E> {}


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


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bits<const N: u32>(usize);

impl<const N: u32> Bits<N> {
    #[doc(hidden)]
    pub const FITS: () = assert!(N <= usize::BITS);
    pub const MASK: usize = (1 << N) - 1;
    const PTR_SHIFT: u32 = usize::BITS - N;

    pub const fn new(value: usize) -> Option<Self> {
        if value & Self::MASK != value {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn new_masked(value: usize) -> Self {
        Self(value & Self::MASK)
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

unsafe impl<const N: u32> Pointer for Bits<N> {
    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value.0 << Self::PTR_SHIFT)
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        Self(ptr.addr() >> Self::PTR_SHIFT)
    }
}

unsafe impl<const N: u32> Aligned for Bits<N> {
    const ALIGNMENT: usize = 1 << Self::PTR_SHIFT;
}

unsafe impl<const N: u32> CloneInPlace for Bits<N> {}


unsafe impl<P: Pointer + Aligned, const N: u32> Pointer for (P, Bits<N>) {
    fn into_ptr(value: Self) -> *const () {
        let ptr = P::into_ptr(value.0);
        let tag = value.1.value() << Self::ALIGNMENT.trailing_zeros();
        ptr.map_addr(|a| a | tag)
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        let ptr_mask = !(P::ALIGNMENT - 1);
        let value = P::from_ptr(ptr.mask(ptr_mask));
        let tag = Bits::<N>::new_masked(ptr.addr() >> Self::ALIGNMENT.trailing_zeros());
        (value, tag)
    }
}

unsafe impl<P: NonNull, const N: u32> NonNull for (P, Bits<N>) {}

unsafe impl<P: Aligned, const N: u32> Aligned for (P, Bits<N>) {
    const ALIGNMENT: usize = P::ALIGNMENT >> N;
}

unsafe impl<P: CloneInPlace, const N: u32> CloneInPlace for (P, Bits<N>) {}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Maybe<T>(pub Option<T>);

impl<T> From<Option<T>> for Maybe<T> {
    fn from(value: Option<T>) -> Self {
        Self(value)
    }
}

impl<T> From<Maybe<T>> for Option<T> {
    fn from(value: Maybe<T>) -> Self {
        value.0
    }
}

impl<T> Deref for Maybe<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Maybe<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Borrow<Option<T>> for Maybe<T> {
    fn borrow(&self) -> &Option<T> {
        &self.0
    }
}

unsafe impl<T: Pointer + NonNull> Pointer for Maybe<T> {
    fn into_ptr(value: Self) -> *const () {
        match value.into() {
            Some(x) => T::into_ptr(x),
            None => ptr::null(),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        if ptr.is_null() {
            Self(None)
        } else {
            Self(Some(unsafe { T::from_ptr(ptr) }))
        }
    }
}

unsafe impl<T: Aligned> Aligned for Maybe<T> {
    const ALIGNMENT: usize = T::ALIGNMENT;
}

unsafe impl<T: CloneInPlace> CloneInPlace for Maybe<T> {}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Null;

unsafe impl Pointer for Null {
    fn into_ptr(_: Self) -> *const () {
        ptr::null()
    }

    unsafe fn from_ptr(ptr: *const ()) -> Self {
        debug_assert!(ptr.is_null());
        Null
    }
}

unsafe impl Aligned for Null {
    const ALIGNMENT: usize = 1 << (usize::BITS - 1);
}

unsafe impl CloneInPlace for Null {}


pub(crate) const fn min(x: usize, y: usize) -> usize {
    if x < y {
        x
    } else {
        y
    }
}

pub(crate) const fn max(x: usize, y: usize) -> usize {
    if x > y {
        x
    } else {
        y
    }
}
