use core::{
    borrow::{Borrow, BorrowMut},
    fmt::Debug,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use crate::{MaybeOwned, Pointer};


pub struct Compact<P: Pointer>(*const (), PhantomData<P>);

impl<P: Pointer> Compact<P> {
    pub fn new(value: P) -> Self {
        Self(P::into_ptr(value), PhantomData)
    }

    pub fn into_inner(self) -> P {
        unsafe { P::from_ptr(self.0).assume_owned() }
    }

    pub fn get_ref(&self) -> Ref<P> {
        Ref(unsafe { P::from_ptr(self.0) }, PhantomData)
    }

    pub fn get_mut(&mut self) -> RefMut<P> {
        RefMut(
            ManuallyDrop::new(unsafe { P::from_ptr(self.0).assume_owned() }),
            self,
        )
    }
}

impl<P: Pointer> Drop for Compact<P> {
    fn drop(&mut self) {
        unsafe { P::from_ptr(self.0).assume_owned() };
    }
}

impl<P: Pointer + Debug> Debug for Compact<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = unsafe { P::from_ptr(self.0) };
        f.debug_tuple("Compact").field(value.deref()).finish()
    }
}

impl<P: Pointer + Clone> Clone for Compact<P> {
    fn clone(&self) -> Self {
        let value = unsafe { P::from_ptr(self.0) };
        Self::new(value.deref().clone())
    }
}


pub struct Ref<'a, P>(MaybeOwned<P>, PhantomData<&'a P>);

impl<'a, P: Pointer> Deref for Ref<'a, P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, P: Pointer> Borrow<P> for Ref<'a, P> {
    fn borrow(&self) -> &P {
        self.0.borrow()
    }
}


pub struct RefMut<'a, P: Pointer>(ManuallyDrop<P>, &'a mut Compact<P>);

impl<'a, P: Pointer> Drop for RefMut<'a, P> {
    fn drop(&mut self) {
        self.1 .0 = P::as_ptr(&self.0);
    }
}

impl<'a, P: Pointer> Deref for RefMut<'a, P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, P: Pointer> DerefMut for RefMut<'a, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

impl<'a, P: Pointer> Borrow<P> for RefMut<'a, P> {
    fn borrow(&self) -> &P {
        self.0.borrow()
    }
}

impl<'a, P: Pointer> BorrowMut<P> for RefMut<'a, P> {
    fn borrow_mut(&mut self) -> &mut P {
        self.0.borrow_mut()
    }
}
