#![feature(associated_const_equality)]
#![feature(doc_cfg)]
#![feature(ptr_mask)]
#![feature(strict_provenance)]
#![warn(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]

use core::{
    borrow::Borrow,
    marker::PhantomData,
    mem,
    mem::{align_of, ManuallyDrop},
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr,
};

#[cfg(feature = "std")]
#[doc(cfg(feature = "std"))]
mod std;

#[cfg(feature = "triomphe")]
#[doc(cfg(feature = "triomphe"))]
mod triomphe;


pub unsafe trait Pointer: Sized {
    const NON_NULL: bool = false;
    const ALIGNMENT: usize = 1;
    const CLONE_IN_PLACE: bool = false;

    fn into_ptr(value: Self) -> *const ();
    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self>;
}


pub trait NonNull: Pointer<NON_NULL = true> {}
impl<T: Pointer<NON_NULL = true>> NonNull for T {}

pub trait VerifyAlignment<const N: usize>: Pointer {
    const VALID: bool = Self::ALIGNMENT.is_power_of_two() && N.is_power_of_two();
    const SUFFICIENT: bool = Self::ALIGNMENT >= N;
}

impl<T: Pointer, const N: usize> VerifyAlignment<N> for T {}

pub trait AlignedTo<const N: usize>: VerifyAlignment<N, VALID = true, SUFFICIENT = true> {}

impl<T, const N: usize> AlignedTo<N> for T where
    T: VerifyAlignment<N, VALID = true, SUFFICIENT = true>
{
}

pub trait CloneInPlace: Pointer<CLONE_IN_PLACE = true> + Clone {
    unsafe fn clone_in_place(ptr: *const ()) {
        let value = unsafe { Self::from_ptr(ptr) };
        mem::forget(value.clone());
    }
}

impl<T: Pointer<CLONE_IN_PLACE = true> + Clone> CloneInPlace for T {}

pub trait Eval {
    const RESULT: bool;
}


#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaybeOwned<T>(ManuallyDrop<T>);

impl<T> MaybeOwned<T> {
    pub const fn new(x: T) -> Self {
        Self(ManuallyDrop::new(x))
    }

    pub const unsafe fn assume_owned(self) -> T {
        ManuallyDrop::into_inner(self.0)
    }

    pub unsafe fn map<U>(self, f: impl FnOnce(T) -> U) -> MaybeOwned<U> {
        MaybeOwned::new(f(unsafe { self.assume_owned() }))
    }
}

impl<T> Deref for MaybeOwned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}


unsafe impl<T> Pointer for *const T {
    fn into_ptr(value: Self) -> *const () {
        value.cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(ptr.cast())
    }
}


unsafe impl<T> Pointer for ptr::NonNull<T> {
    const NON_NULL: bool = true;

    fn into_ptr(value: Self) -> *const () {
        value.as_ptr() as *const ()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { ptr::NonNull::new_unchecked(ptr as *mut T) })
    }
}


unsafe impl<T> Pointer for &'static T {
    const NON_NULL: bool = true;
    const ALIGNMENT: usize = align_of::<T>();
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        ptr::from_ref(value).cast()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { &*ptr.cast() })
    }
}


unsafe impl<T> Pointer for Option<T>
where
    T: Pointer + AlignedTo<2>,
{
    const NON_NULL: bool = T::NON_NULL;
    const ALIGNMENT: usize = T::ALIGNMENT >> 1;
    const CLONE_IN_PLACE: bool = T::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        match value {
            Some(x) => T::into_ptr(x),
            None => ptr::without_provenance(Self::ALIGNMENT),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        let tag = ptr.addr() & Self::ALIGNMENT;
        let ptr = ptr.mask(!((Self::ALIGNMENT << 1) - 1));

        if tag == 0 {
            unsafe { T::from_ptr(ptr).map(Some) }
        } else {
            MaybeOwned::new(None)
        }
    }
}


unsafe impl<T, E> Pointer for Result<T, E>
where
    T: Pointer + AlignedTo<2>,
    E: Pointer + AlignedTo<2>,
{
    const NON_NULL: bool = T::NON_NULL;
    const ALIGNMENT: usize = min(T::ALIGNMENT, E::ALIGNMENT) >> 1;
    const CLONE_IN_PLACE: bool = T::CLONE_IN_PLACE && E::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        let (ptr, tag) = match value {
            Ok(x) => (T::into_ptr(x), 0),
            Err(x) => (E::into_ptr(x), Self::ALIGNMENT),
        };

        ptr.map_addr(|a| a | tag)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        let tag = ptr.addr() & Self::ALIGNMENT;
        let ptr = ptr.mask(!((Self::ALIGNMENT << 1) - 1));

        if tag == 0 {
            unsafe { T::from_ptr(ptr).map(Ok) }
        } else {
            unsafe { E::from_ptr(ptr).map(Err) }
        }
    }
}


unsafe impl Pointer for usize {
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(ptr.addr())
    }
}


unsafe impl Pointer for NonZeroUsize {
    const NON_NULL: bool = true;
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value.into())
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(unsafe { NonZeroUsize::new_unchecked(ptr.addr()) })
    }
}


unsafe impl Pointer for () {
    const NON_NULL: bool = true;
    const ALIGNMENT: usize = 1 << (usize::BITS - 1);
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(_: Self) -> *const () {
        ptr::without_provenance(Self::ALIGNMENT)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        debug_assert!(ptr.addr() == Self::ALIGNMENT);
        MaybeOwned::new(())
    }
}


pub struct FitsInUsize<const BITS: u32>;

impl<const BITS: u32> Eval for FitsInUsize<BITS> {
    const RESULT: bool = BITS <= usize::BITS;
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bits<const N: u32>(usize);

impl<const N: u32> Bits<N>
where
    FitsInUsize<N>: Eval<RESULT = true>,
{
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

unsafe impl<const N: u32> Pointer for Bits<N>
where
    FitsInUsize<N>: Eval<RESULT = true>,
{
    const ALIGNMENT: usize = 1 << Self::PTR_SHIFT;
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value.0 << Self::PTR_SHIFT)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(Self(ptr.addr() >> Self::PTR_SHIFT))
    }
}


pub struct FreeBits<P, const N: u32>(PhantomData<P>);

impl<P: Pointer, const N: u32> Eval for FreeBits<P, N> {
    const RESULT: bool = P::ALIGNMENT >= (1 << N);
}

unsafe impl<P, const N: u32> Pointer for (P, Bits<N>)
where
    P: Pointer,
    FitsInUsize<N>: Eval<RESULT = true>,
    FreeBits<P, N>: Eval<RESULT = true>,
{
    const NON_NULL: bool = P::NON_NULL;
    const ALIGNMENT: usize = P::ALIGNMENT >> N;
    const CLONE_IN_PLACE: bool = P::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        let ptr = P::into_ptr(value.0);
        let tag = value.1.value() << Self::ALIGNMENT.trailing_zeros();
        ptr.map_addr(|a| a | tag)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        let tag = Bits::<N>::new_masked(ptr.addr() >> Self::ALIGNMENT.trailing_zeros());
        let ptr = ptr.mask(!(P::ALIGNMENT - 1));
        unsafe { P::from_ptr(ptr).map(|p| (p, tag)) }
    }
}


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

unsafe impl<T: Pointer<NON_NULL = true>> Pointer for Maybe<T> {
    const ALIGNMENT: usize = T::ALIGNMENT;
    const CLONE_IN_PLACE: bool = T::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        match value.into() {
            Some(x) => T::into_ptr(x),
            None => ptr::null(),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        if ptr.is_null() {
            MaybeOwned::new(Self(None))
        } else {
            unsafe { T::from_ptr(ptr).map(|p| Self(Some(p))) }
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Null;

unsafe impl Pointer for Null {
    const ALIGNMENT: usize = 1 << (usize::BITS - 1);
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(_: Self) -> *const () {
        ptr::null()
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        debug_assert!(ptr.is_null());
        MaybeOwned::new(Null)
    }
}


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
