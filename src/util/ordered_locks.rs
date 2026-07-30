use std::{marker::PhantomData, sync::Arc};

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

pub trait ReadAllowed<I: Nat> {}
pub trait WriteAllowed<I: Nat> {}

/// This is really only an [Arc] around M. All other data is erased at compilation.
pub struct OrderedLocks<T, M, L: Nat> {
    inner: Arc<M>,
    order: PhantomData<(T, L)>,
}

// impl<T, M, L: Nat> Deref for OrderedLocks<T, M, L> {
//     type Target = M;
//     fn deref(&self) -> &Self::Target {
//         &*self.inner
//     }
// }

impl<T, M> OrderedLocks<T, M, Zero> {
    /// This function is unsafe because the whole effectiveness of the lock ordering approach rests
    /// on there only being one [OrderedLocks] object through which the locks are accessed.
    pub unsafe fn new_zero(inner: Arc<M>) -> Self {
        Self {
            inner,
            order: PhantomData,
        }
    }
}

impl<T, M, L: Nat> OrderedLocks<T, M, L> {
    /// This function is unsafe because it can be used to access the locks directly, breaking the ordering.
    pub unsafe fn inner(&self) -> &M {
        &*self.inner
    }

    /// This function is unsafe because it can be used to access the locks directly, breaking the ordering.
    pub unsafe fn inner_arc(&self) -> Arc<M> {
        self.inner.clone()
    }

    /// This function is unsafe because it allows to change the integer `L`, which allows breaking
    /// the ordering.
    pub unsafe fn new(inner: Arc<M>) -> Self {
        Self {
            inner,
            order: PhantomData,
        }
    }
}

impl<T, M, L: Nat> OrderedLocks<T, M, L> {
    #[inline]
    pub fn ith<I, R, X>(self, mut f: impl FnMut(&X, OrderedLocks<T, M, Succ<I>>) -> R) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: Access<I, X>,
        T: ReadAllowed<I>,
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
        mut f: impl FnMut(&X, OrderedLocks<T, M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccess<I, Ix, X>,
        T: ReadAllowed<I>,
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
        mut f: impl FnMut(&X, &X, OrderedLocks<T, M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccess<I, Ix, X>,
        Ix: Copy,
        T: ReadAllowed<I>,
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
        mut f: impl FnMut(&mut X, OrderedLocks<T, M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: AccessMut<I, X>,
        T: WriteAllowed<I>,
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
        mut f: impl FnMut(&mut X, OrderedLocks<T, M, Succ<I>>) -> R,
    ) -> (R, Self)
    where
        I: Nat,
        L: AtMost<I>,
        M: IndexedAccessMut<I, Ix, X>,
        T: WriteAllowed<I>,
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

// macro_rules! impl_access_forward {
//     (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $result:ty, $fieldname:ident) => {
//         impl<$($t:$tr),*> $crate::util::ordered_locks::Access<$level, $result> for $domain {
//             #[inline]
//             unsafe fn with<R>(&self, f: impl FnMut(&$result) -> R) -> R {
//                 <_ as $crate::util::ordered_locks::Access<$level, $result>>::with(&self.$fieldname, f)
//             }
//         }
//     };
// }
// pub(crate) use impl_access_forward;

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

// macro_rules! impl_access_mut_forward {
//     (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $result:ty, $fieldname:ident) => {
//         impl<$($t:$tr),*> $crate::util::ordered_locks::AccessMut<$level, $result> for $domain {
//             #[inline]
//             unsafe fn with_mut<R>(&self, f: impl FnMut(&mut $result) -> R) -> R {
//                 <_ as $crate::util::ordered_locks::AccessMut<$level, $result>>::with_mut(&self.$fieldname, f)
//             }
//         }
//     };
// }
// pub(crate) use impl_access_mut_forward;

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

// macro_rules! impl_indexed_access_forward {
//     (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $index:ty, $result:ty, $fieldname:ident) => {
//         impl<$($t:$tr),*> $crate::util::ordered_locks::IndexedAccess<$level, $index, $result> for $domain {
//             #[inline]
//             unsafe fn with_index<R>(&self, i: $index, f: impl FnMut(&$result) -> R) -> R {
//                 <_ as $crate::util::ordered_locks::IndexedAccess<$level, $index, $result>>::with_index(&self.$fieldname, i, f)
//             }
//         }
//     };
// }
// pub(crate) use impl_indexed_access_forward;

macro_rules! impl_indexed_access_mut {
    (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $index:ty, $result:ty, |$self:ident, $i:ident| $x:expr) => {
        impl<$($t:$tr),*> $crate::util::ordered_locks::IndexedAccessMut<$level, $index, $result> for $domain {
            #[inline]
            unsafe fn with_index_mut<R>(&$self, $i: $index, mut f: impl FnMut(&mut $result) -> R) -> R {
                f($x)
            }
        }
    };
}
pub(crate) use impl_indexed_access_mut;

// macro_rules! impl_indexed_access_mut_forward {
//     (< $(  $t:ident : $tr:path  ),* >, $domain:ty, $level:ty, $index:ty, $result:ty, $fieldname:ident) => {
//         impl<$($t:$tr),*> $crate::util::ordered_locks::IndexedAccessMut<$level, $index, $result> for $domain {
//             #[inline]
//             unsafe fn with_index_mut<R>(&self, i: $index, f: impl FnMut(&mut $result) -> R) -> R {
//                 <_ as $crate::util::ordered_locks::IndexedAccessMut<$level, $index, $result>>::with_index_mut(&self.$fieldname, i, f)
//             }
//         }
//     };
// }
// pub(crate) use impl_indexed_access_mut_forward;
