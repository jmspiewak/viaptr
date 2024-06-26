#![feature(associated_const_equality)]
#![feature(doc_cfg)]
#![feature(ptr_mask)]
#![feature(strict_provenance)]
#![warn(unsafe_op_in_unsafe_fn)]
#![no_std]
#![doc = include_str!("../README.md")]

use core::{
    borrow::Borrow,
    marker::PhantomData,
    mem,
    mem::{align_of, ManuallyDrop},
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr,
};

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod compact;
pub mod shy_atomic;

#[cfg(feature = "alloc")]
#[doc(cfg(feature = "alloc"))]
mod impl_alloc;

#[cfg(feature = "triomphe")]
#[doc(cfg(feature = "triomphe"))]
mod impl_triomphe;

/// Conversion to and from `*const ()`.
pub unsafe trait Pointer: Sized {
    const NON_NULL: bool = false;
    const ALIGNMENT: usize = 1;
    const CLONE_IN_PLACE: bool = false;

    fn into_ptr(value: Self) -> *const ();
    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self>;

    fn as_ptr(value: &Self) -> *const () {
        Self::into_ptr(unsafe { ptr::read(value) })
    }
}


/// Require non-null pointers from [`Pointer::into_ptr`].
pub trait NonNull: Pointer<NON_NULL = true> {}
impl<T: Pointer<NON_NULL = true>> NonNull for T {}

/// Verify [`Pointer`] alignment validity and magnitude.
pub trait VerifyAlignment<const N: usize>: Pointer {
    const VALID: bool = Self::ALIGNMENT.is_power_of_two() && N.is_power_of_two();
    const SUFFICIENT: bool = Self::ALIGNMENT >= N;
}

impl<T: Pointer, const N: usize> VerifyAlignment<N> for T {}

/// Require minimum [`Pointer`] alignment.
pub trait AlignedTo<const N: usize>: VerifyAlignment<N, VALID = true, SUFFICIENT = true> {}

impl<T, const N: usize> AlignedTo<N> for T where
    T: VerifyAlignment<N, VALID = true, SUFFICIENT = true>
{
}

/// A trait for types which can be cloned in place.
pub trait CloneInPlace: Pointer<CLONE_IN_PLACE = true> + Clone {
    fn clone_in_place(value: &Self) {
        mem::forget(value.clone());
    }

    unsafe fn clone_from_ptr(ptr: *const ()) -> Self {
        unsafe { Self::from_ptr(ptr) }.clone()
    }

    unsafe fn clone_by_ptr(ptr: *const ()) {
        mem::forget(unsafe { Self::clone_from_ptr(ptr) })
    }

    unsafe fn drop_one(ptr: *const ()) {
        unsafe { Self::from_ptr(ptr).assume_owned() };
    }
}

impl<T: Pointer<CLONE_IN_PLACE = true> + Clone> CloneInPlace for T {}

/// Predicate evaluation trait.
pub trait Eval {
    const RESULT: bool;
}


/// A wrapper type to construct values which might not own their contents.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaybeOwned<T>(ManuallyDrop<T>);

impl<T> MaybeOwned<T> {
    pub const fn new(x: T) -> Self {
        Self(ManuallyDrop::new(x))
    }

    pub const unsafe fn assume_owned(self) -> T {
        ManuallyDrop::into_inner(self.0)
    }

    pub unsafe fn drop(slot: &mut Self) {
        unsafe { ManuallyDrop::drop(&mut slot.0) };
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

impl<T> Borrow<T> for MaybeOwned<T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
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

unsafe impl<T: Pointer<NON_NULL = true>> Pointer for Option<T> {
    const ALIGNMENT: usize = T::ALIGNMENT;
    const CLONE_IN_PLACE: bool = T::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        match value {
            Some(x) => T::into_ptr(x),
            None => ptr::null(),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        if ptr.is_null() {
            MaybeOwned::new(None)
        } else {
            unsafe { T::from_ptr(ptr).map(Some) }
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


/// A predicate checking if [`usize`] has at least `N` bits.
pub struct FitsInUsize<const N: u32>;

impl<const N: u32> Eval for FitsInUsize<N> {
    const RESULT: bool = N <= usize::BITS;
}


/// Unsigned integers at most `N` bits long.
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


/// A predicate checking if `P` is aligned enough to fit `N` bits.
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
        let tag = Bits::new_masked(ptr.addr() >> Self::ALIGNMENT.trailing_zeros());
        let ptr = ptr.mask(!(P::ALIGNMENT - 1));
        unsafe { P::from_ptr(ptr).map(|p| (p, tag)) }
    }
}


/// Unsigned integer, always less than `N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Num<const N: usize>(usize);

impl<const N: usize> Num<N> {
    pub const MASK: usize = N.next_power_of_two() - 1;
    const PTR_SHIFT: u32 = usize::BITS - Self::MASK.trailing_ones();

    pub const fn new(value: usize) -> Option<Self> {
        if value >= N {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn new_saturating(value: usize) -> Self {
        Self(min(value, N - 1))
    }

    pub const fn new_wrapping(value: usize) -> Self {
        Self(value % N)
    }

    pub const unsafe fn new_unchecked(value: usize) -> Self {
        Self(value)
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

unsafe impl<const N: usize> Pointer for Num<N> {
    const ALIGNMENT: usize = 1 << Self::PTR_SHIFT;
    const CLONE_IN_PLACE: bool = true;

    fn into_ptr(value: Self) -> *const () {
        ptr::without_provenance(value.0 << Self::PTR_SHIFT)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        MaybeOwned::new(Self(ptr.addr() >> Self::PTR_SHIFT))
    }
}


/// A predicate checking if `P` is aligned enough to fit an unsigned int less than `N`.
pub struct CanFitNum<P, const N: usize>(PhantomData<P>);

impl<P: Pointer, const N: usize> Eval for CanFitNum<P, N> {
    const RESULT: bool = P::ALIGNMENT >= N;
}

unsafe impl<P, const N: usize> Pointer for (P, Num<N>)
where
    P: Pointer,
    CanFitNum<P, N>: Eval<RESULT = true>,
{
    const NON_NULL: bool = P::NON_NULL;
    const ALIGNMENT: usize = P::ALIGNMENT / N.next_power_of_two();
    const CLONE_IN_PLACE: bool = P::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        let ptr = P::into_ptr(value.0);
        let tag = value.1.value() << Self::ALIGNMENT.trailing_zeros();
        ptr.map_addr(|a| a | tag)
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        let tag = (ptr.addr() >> Self::ALIGNMENT.trailing_zeros()) & Num::<N>::MASK;
        let tag = unsafe { Num::new_unchecked(tag) };
        let ptr = ptr.mask(!(P::ALIGNMENT - 1));
        unsafe { P::from_ptr(ptr).map(|p| (p, tag)) }
    }
}


/// Like [`Option`], but preserves [`Pointer`] implementation when nested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NestOption<T>(pub Option<T>);

impl<T> From<Option<T>> for NestOption<T> {
    fn from(value: Option<T>) -> Self {
        Self(value)
    }
}

impl<T> From<NestOption<T>> for Option<T> {
    fn from(value: NestOption<T>) -> Self {
        value.0
    }
}

impl<T> Deref for NestOption<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for NestOption<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Borrow<Option<T>> for NestOption<T> {
    fn borrow(&self) -> &Option<T> {
        &self.0
    }
}

unsafe impl<T> Pointer for NestOption<T>
where
    T: Pointer + AlignedTo<2>,
{
    const NON_NULL: bool = T::NON_NULL;
    const ALIGNMENT: usize = T::ALIGNMENT >> 1;
    const CLONE_IN_PLACE: bool = T::CLONE_IN_PLACE;

    fn into_ptr(value: Self) -> *const () {
        match value.into() {
            Some(x) => T::into_ptr(x),
            None => ptr::without_provenance(Self::ALIGNMENT),
        }
    }

    unsafe fn from_ptr(ptr: *const ()) -> MaybeOwned<Self> {
        let tag = ptr.addr() & Self::ALIGNMENT;
        let ptr = ptr.mask(!((Self::ALIGNMENT << 1) - 1));

        if tag == 0 {
            unsafe { T::from_ptr(ptr).map(|p| Self(Some(p))) }
        } else {
            MaybeOwned::new(Self(None))
        }
    }
}


/// A value always encoded as a null pointer.
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
