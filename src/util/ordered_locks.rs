use std::{marker::PhantomData, ops::Deref, rc::Rc};

pub unsafe trait Nat {}
pub struct Zero;
unsafe impl Nat for Zero {}
pub struct Succ<N: Nat>(PhantomData<N>);
unsafe impl<N: Nat> Nat for Succ<N> {}

/// Users are not allowed to implement AtMost.
pub unsafe trait AtMost<N: Nat>: Nat {}
unsafe impl AtMost<Zero> for Zero {}
unsafe impl<N: Nat> AtMost<Succ<N>> for Zero {}
unsafe impl<N: Nat, M: AtMost<N>> AtMost<Succ<N>> for Succ<M> {}

/// This is really only a reference to M. All other data is erased at compilation.
pub struct OrderedLocks<M, L: Nat> {
    inner: Rc<M>,
    order: PhantomData<L>,
}

impl<M, L: Nat> Deref for OrderedLocks<M, L> {
    type Target = M;
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}
impl<M> OrderedLocks<M, Zero> {
    /// This function is unsafe because the whole effetiveness of the lock ordering approach rests
    /// on there only being one [OrderedLocks] object through which the locks are accessed.
    pub unsafe fn zero(inner: Rc<M>) -> Self {
        Self {
            inner,
            order: PhantomData,
        }
    }
}

impl<M, L: Nat> OrderedLocks<M, L> {
    unsafe fn new(inner: Rc<M>) -> Self {
        Self {
            inner,
            order: PhantomData,
        }
    }
}

impl<M, L: Nat> OrderedLocks<M, L> {
    #[inline]
    pub fn ith<I, R, X>(self, mut f: impl FnMut(&X, OrderedLocks<M, Succ<I>>) -> R) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: Access<I, X>,
    {
        (
            unsafe {
                self.inner
                    .with(|x| f(x, OrderedLocks::new(self.inner.clone())))
            },
            self,
        )
    }

    #[inline]
    pub fn ith_indexed<I, R, Ix, X>(
        self,
        ix: Ix,
        mut f: impl FnMut(&X, OrderedLocks<M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccess<I, Ix, X>,
    {
        (
            unsafe {
                self.inner
                    .with_index(ix, |x| f(x, OrderedLocks::new(self.inner.clone())))
            },
            self,
        )
    }

    #[inline]
    pub fn ith_indexed_pair<I, R, Ix, X>(
        self,
        ix: Ix,
        jx: Ix,
        mut f: impl FnMut(&X, &X, OrderedLocks<M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccess<I, Ix, X>,
        Ix: Copy,
    {
        (
            unsafe {
                self.inner.with_index(ix, |x| {
                    self.inner
                        .with_index(jx, |y| f(x, y, OrderedLocks::new(self.inner.clone())))
                })
            },
            self,
        )
    }

    #[inline]
    pub fn ith_mut<I, R, X>(
        self,
        mut f: impl FnMut(&mut X, OrderedLocks<M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: AccessMut<I, X>,
    {
        (
            unsafe {
                self.inner
                    .with_mut(|x| f(x, OrderedLocks::new(self.inner.clone())))
            },
            self,
        )
    }

    #[inline]
    pub fn ith_indexed_mut<I, R, Ix, X>(
        self,
        ix: Ix,
        mut f: impl FnMut(&mut X, OrderedLocks<M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccessMut<I, Ix, X>,
    {
        (
            unsafe {
                self.inner
                    .with_index_mut(ix, |x| f(x, OrderedLocks::new(self.inner.clone())))
            },
            self,
        )
    }
}

pub trait Access<L: Nat, X> {
    unsafe fn with<R>(&self, f: impl FnMut(&X) -> R) -> R;
}

pub trait IndexedAccess<L: Nat, Ix, X> {
    unsafe fn with_index<R>(&self, i: Ix, f: impl FnMut(&X) -> R) -> R;
}

pub trait AccessMut<L: Nat, X> {
    unsafe fn with_mut<R>(&self, f: impl FnMut(&mut X) -> R) -> R;
}

pub trait IndexedAccessMut<L: Nat, Ix, X> {
    unsafe fn with_index_mut<R>(&self, i: Ix, f: impl FnMut(&mut X) -> R) -> R;
}

macro_rules! impl_access {
    (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $result:ty, |$self:ident| $x:expr) => {
        impl<$($t:$tr),*> $crate::util::ordered_locks::Access<$level, $result> for $domain {
            #[inline]
            unsafe fn with<R>(&$self, mut f: impl FnMut(&$result) -> R) -> R {
                f($x)
            }
        }
    };
}
pub(crate) use impl_access;

macro_rules! impl_access_mut {
    (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $result:ty, |$self:ident| $x:expr) => {
        impl<$($t:$tr),*> $crate::util::ordered_locks::AccessMut<$level, $result> for $domain {
            #[inline]
            unsafe fn with_mut<R>(&$self, mut f: impl FnMut(&mut $result) -> R) -> R {
                f($x)
            }
        }
    };
}
pub(crate) use impl_access_mut;

macro_rules! impl_indexed_access {
    (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $index:ty, $result:ty, |$self:ident, $i:ident| $x:expr) => {
        impl<$($t:$tr),*> $crate::util::ordered_locks::IndexedAccess<$level, $index, $result> for $domain {
            #[inline]
            unsafe fn with_index<R>(&$self, $i: $index, mut f: impl FnMut(&$result) -> R) -> R {
                f($x)
            }
        }
    };
}
pub(crate) use impl_indexed_access;

macro_rules! impl_indexed_access_mut {
    (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $index:ty, $result:ty, |$self:ident, $i:ident| $x:expr) => {
        impl<$($t:$tr),*> $crate::util::ordered_locks::IndexedAccessMut<$level, $index, $result> for $domain {
            #[inline]
            unsafe fn with_index<R>(&$self, $i: $index, mut f: impl FnMut(&mut $result) -> R) -> R {
                f($x)
            }
        }
    };
}
pub(crate) use impl_indexed_access_mut;
