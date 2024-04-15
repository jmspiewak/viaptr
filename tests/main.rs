#![feature(pointer_is_aligned)]

use std::{fmt::Debug, num::NonZeroUsize, ptr, rc, rc::Rc, sync, sync::Arc};

use proptest::{
    bits, num,
    num::usize,
    option::OptionStrategy,
    proptest,
    result::MaybeOk,
    sample::{select, Select},
    strategy::{Just, Strategy},
};
use viaptr::{Aligned, Bits, Maybe, NonNull, Null, Pointer};


fn roundtrip<T: Pointer + Debug + Clone + PartialEq>(x: T) {
    let ptr = T::into_ptr(x.clone());
    let y = unsafe { T::from_ptr(ptr).assume_owned() };
    assert_eq!(x, y);
}

fn non_null<T: Pointer + NonNull>(x: T) {
    let ptr = T::into_ptr(x);
    assert!(!ptr.is_null());
    unsafe { T::from_ptr(ptr).assume_owned() };
}

fn aligned<T: Pointer + Aligned>(x: T) {
    let ptr = T::into_ptr(x);
    assert!(ptr.is_aligned_to(T::ALIGNMENT));
    unsafe { T::from_ptr(ptr).assume_owned() };
}


const PTRS: [*const usize; 4] = [
    ptr::from_ref(&0),
    ptr::from_ref(&1),
    ptr::from_ref(&2),
    ptr::from_ref(&3),
];

const NON_NULLS: [ptr::NonNull<usize>; 4] = [
    unsafe { ptr::NonNull::new_unchecked(PTRS[0] as *mut _) },
    unsafe { ptr::NonNull::new_unchecked(PTRS[1] as *mut _) },
    unsafe { ptr::NonNull::new_unchecked(PTRS[2] as *mut _) },
    unsafe { ptr::NonNull::new_unchecked(PTRS[3] as *mut _) },
];

const REFS: [&usize; 8] = [&0, &1, &2, &3, &4, &5, &6, &7];

fn some_ptr() -> Select<*const usize> {
    select(&PTRS)
}

fn some_non_null() -> Select<ptr::NonNull<usize>> {
    select(&NON_NULLS)
}

fn some_ref() -> Select<&'static usize> {
    select(&REFS)
}

fn option<T: Strategy>(x: T) -> OptionStrategy<T> {
    proptest::option::of(x)
}

fn result<T: Strategy, E: Strategy>(ok: T, err: E) -> MaybeOk<T, E> {
    proptest::result::maybe_ok(ok, err)
}

fn usize() -> num::usize::Any {
    num::usize::ANY
}

fn non_zero() -> impl Strategy<Value = NonZeroUsize> {
    usize().prop_filter_map("zero", NonZeroUsize::new)
}

fn unit() -> Just<()> {
    Just(())
}

fn bits<const N: u32>() -> impl Strategy<Value = Bits<N>> {
    bits::usize::masked(Bits::<N>::MASK).prop_map(Bits::<N>::new_masked)
}

fn maybe<T: Strategy>(x: T) -> impl Strategy<Value = Maybe<T::Value>> {
    option(x).prop_map(From::from)
}

fn null() -> Just<Null> {
    Just(Null)
}

fn boxed<T: Strategy>(x: T) -> impl Strategy<Value = Box<T::Value>> {
    x.prop_map(Box::new)
}

fn rc<T: Strategy>(x: T) -> impl Strategy<Value = Rc<T::Value>> {
    x.prop_map(Rc::new)
}

fn arc<T: Strategy>(x: T) -> impl Strategy<Value = Arc<T::Value>> {
    x.prop_map(Arc::new)
}


macro_rules! gen {
    ($($name:ident ($($test:ident),+) $strategy:expr;)+) => {
        proptest!{$(
            #[test]
            fn $name(x in $strategy) {
                $(gen!(@$test x);)+
            }
        )+}
    };

    (@R $x:ident) => {
        roundtrip(Clone::clone(&$x))
    };

    (@N $x:ident) => {
        non_null(Clone::clone(&$x))
    };

    (@A $x:ident) => {
        aligned($x)
    };
}

gen! {
    basic1 (R) some_ptr();
    basic2 (R, N) some_non_null();
    basic3 (R, N, A) some_ref();
    basic4 (R, N, A) option(some_ref());
    basic5 (R, N, A) result(some_ref(), some_ref());
    basic6 (R) usize();
    basic7 (R, N) non_zero();
    basic8 (R, N, A) unit();
    basic9 (R, A) bits::<5>();
    basic10 (R, N, A) (some_ref(), bits::<2>());
    basic11 (R, A) maybe(some_ref());
    basic12 (R, A) null();
    basic13 (R, N, A) boxed(usize());
    basic14 (R, N, A) rc(usize());
    basic15 (R, N, A) arc(usize());
}

proptest! {
    #[test]
    fn weak1(src in rc(usize())) {
        type T = rc::Weak<usize>;
        let x = Rc::downgrade(&src);
        let ptr = T::into_ptr(x);
        let y = unsafe { T::from_ptr(ptr).assume_owned() };
        assert_eq!(y.upgrade(), Some(src))
    }

    #[test]
    fn weak2(src in arc(usize())) {
        type T = sync::Weak<usize>;
        let x = Arc::downgrade(&src);
        let ptr = T::into_ptr(x);
        let y = unsafe { T::from_ptr(ptr).assume_owned() };
        assert_eq!(y.upgrade(), Some(src))
    }
}


#[cfg(feature = "triomphe")]
mod triomphe {
    use proptest::{
        collection::{vec, VecStrategy},
        proptest,
        strategy::Strategy,
    };
    use triomphe::{Arc, ThinArc};

    use super::{aligned, non_null, roundtrip, usize};


    fn arc<T: Strategy>(x: T) -> impl Strategy<Value = Arc<T::Value>> {
        x.prop_map(Arc::new)
    }

    fn thin_arc<H: Strategy, T: Strategy>(
        h: H,
        t: VecStrategy<T>,
    ) -> impl Strategy<Value = ThinArc<H::Value, T::Value>> {
        (h, t).prop_map(|(h, t)| ThinArc::from_header_and_iter(h, t.into_iter()))
    }


    gen! {
        t1 (R, N, A) arc(usize());
        t2 (R, N, A) thin_arc(usize(), vec(usize(), 0..5));
    }
}
