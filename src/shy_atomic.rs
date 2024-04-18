use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, Ordering::*},
};

use crate::Pointer;


/// An atomic pointer which can't be observed without modification.
/// To get the current value a new one must be supplied.
#[derive(Debug)]
pub struct ShyAtomic<P: Pointer>(AtomicPtr<()>, PhantomData<P>);

impl<P: Pointer> ShyAtomic<P> {
    pub fn new(value: P) -> Self {
        Self(AtomicPtr::new(P::into_ptr(value).cast_mut()), PhantomData)
    }

    pub fn store(&self, value: P) {
        self.swap(value);
    }

    pub fn swap(&self, value: P) -> P {
        let ptr = P::into_ptr(value).cast_mut();
        let ptr = self.0.swap(ptr, AcqRel);
        unsafe { P::from_ptr(ptr).assume_owned() }
    }

    /// Returns back `new` if the exchange fails, not the current value.
    pub fn compare_exchange(&self, cmp: &P, new: P) -> Result<P, P> {
        let cmp = P::as_ptr(cmp).cast_mut();
        let ptr = P::into_ptr(new).cast_mut();

        match self.0.compare_exchange(cmp, ptr, AcqRel, Relaxed) {
            Ok(old) => Ok(unsafe { P::from_ptr(old).assume_owned() }),
            Err(_) => Err(unsafe { P::from_ptr(ptr).assume_owned() }),
        }
    }
}

impl<P: Pointer> Drop for ShyAtomic<P> {
    fn drop(&mut self) {
        unsafe { P::from_ptr(self.0.load(Acquire)).assume_owned() };
    }
}
