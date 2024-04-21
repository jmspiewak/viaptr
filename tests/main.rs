#![feature(associated_const_equality)]
#![feature(pointer_is_aligned_to)]

use std::{fmt::Debug, num::NonZeroUsize, ops::Deref, ptr, rc, rc::Rc, sync, sync::Arc};

use proptest::{
    array::{uniform, UniformArrayStrategy},
    bits, num,
    num::usize,
    option::OptionStrategy,
    proptest,
    result::MaybeOk,
    sample::{select, Select},
    strategy::{Just, Strategy},
};
use viaptr::{
    compact::Compact, shy_atomic::ShyAtomic, AlignedTo, Bits, CloneInPlace, Eval, FitsInUsize,
    NestOption, NonNull, Null, Num, Pointer,
};


fn test_pointer<T: Pointer + Debug + Clone + PartialEq>(x: &T) {
    let ptr = T::into_ptr(x.clone());
    let y = unsafe { T::from_ptr(ptr).assume_owned() };
    assert_eq!(*x, y);

    if T::NON_NULL {
        assert!(!ptr.is_null());
    }

    assert!(ptr.is_aligned_to(T::ALIGNMENT));
}

fn test_non_null<T: NonNull>(_: &T) {}
fn test_aligned<T: AlignedTo<2>>(_: &T) {}


const CIP_ITERS: usize = 10;

fn clone_in_place<T: CloneInPlace + Debug + Clone + PartialEq>(x: &T) {
    let ptr = T::into_ptr(x.clone());

    for _ in 0 .. CIP_ITERS {
        unsafe { T::clone_by_ptr(ptr) };
    }

    for _ in 0 .. CIP_ITERS + 1 {
        let y = unsafe { T::from_ptr(ptr).assume_owned() };
        assert_eq!(*x, y);
    }
}


#[repr(align(64))]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Aligned(usize);

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

fn bits<const N: u32>() -> impl Strategy<Value = Bits<N>>
where
    FitsInUsize<N>: Eval<RESULT = true>,
{
    bits::usize::masked(Bits::<N>::MASK).prop_map(Bits::<N>::new_masked)
}

fn num<const N: usize>() -> impl Strategy<Value = Num<N>> {
    usize().prop_map(Num::new_wrapping)
}

fn nest_option<T: Strategy>(x: T) -> impl Strategy<Value = NestOption<T::Value>> {
    option(x).prop_map(From::from)
}

fn null() -> Just<Null> {
    Just(Null)
}

fn aligned() -> impl Strategy<Value = Aligned> {
    usize().prop_map(Aligned)
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

fn array<T: Strategy, const N: usize>(x: T) -> UniformArrayStrategy<T, [T::Value; N]> {
    uniform(x)
}

fn compound() -> impl Strategy<Value: Pointer + Debug + Clone + PartialEq> {
    result(
        result(arc(usize()), rc(usize())),
        result(option(boxed(usize())), bits::<20>()),
    )
}

fn compound_cip() -> impl Strategy<Value: CloneInPlace + Debug + PartialEq> {
    result(
        result(arc(usize()), rc(usize())),
        result(option(arc(usize())), bits::<20>()),
    )
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

    (@P $x:ident) => {
        test_pointer(&$x)
    };

    (@N $x:ident) => {
        test_non_null(&$x)
    };

    (@A $x:ident) => {
        test_aligned(&$x)
    };

    (@C $x:ident) => {
        clone_in_place(&$x)
    };
}

gen! {
    basic1 (P) some_ptr();
    basic2 (P, N) some_non_null();
    basic3 (P, N, A, C) some_ref();
    basic4 (P, A, C) option(some_ref());
    basic5 (P, N, A, C) result(some_ref(), some_ref());
    basic6 (P, C) usize();
    basic7 (P, N, C) non_zero();
    basic8 (P, N, A, C) unit();
    basic9 (P, A, C) bits::<5>();
    basic10 (P, N, A, C) (some_ref(), bits::<2>());
    basic11 (P, N, A, C) nest_option(some_ref());
    basic12 (P, A, C) null();
    basic13 (P, N, A) boxed(usize());
    basic14 (P, N, A, C) rc(usize());
    basic15 (P, N, A, C) arc(usize());
    basic16 (P, A, C) num::<42>();

    c1 (P) compound();
    c2 (P, C) compound_cip();
    c3 (P, N, C) result((arc(aligned()), num::<30>()), (rc(aligned()), bits::<5>()));
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

    use super::{clone_in_place, test_aligned, test_non_null, test_pointer, usize};


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
        t1 (P, N, A, C) arc(usize());
        t2 (P, N, A, C) thin_arc(usize(), vec(usize(), 0..5));
    }
}


proptest! {
    #[test]
    fn comapct([x, y, z] in array(compound())) {
        let mut c = Compact::new(x.clone());
        assert_eq!(x, c.get_clone());
        assert_eq!(&x, c.get_ref().deref());
        assert_eq!(&x, c.get_mut().deref());

        let old = c.swap(y.clone());
        assert_eq!(x, old);
        assert_eq!(&y, c.get_ref().deref());

        *c.get_mut() = z.clone();
        assert_eq!(z, c.get_clone());
    }

    #[test]
    fn shy_atomic([x, y, z, w] in array(compound_cip())) {
        let a = ShyAtomic::new(x.clone());
        assert_eq!(x, a.swap(y.clone()));
        assert_eq!(y, a.swap(z.clone()));

        if Pointer::as_ptr(&x) != Pointer::as_ptr(&z) {
            assert_eq!(Err(&w), a.compare_exchange(&x, w.clone()).as_ref());
            assert_eq!(Ok(&z), a.compare_exchange(&z, w.clone()).as_ref());
        }
    }
}
