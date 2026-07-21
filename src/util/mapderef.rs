use std::ops::Deref;

use parking_lot::{MappedRwLockReadGuard, RwLockReadGuard};

pub trait MapDeref: Deref {
    fn map<X: 'static>(self, f: impl FnOnce(&Self::Target) -> &X) -> impl MapDeref<Target = X>;
}

impl<'rwlock, T: ?Sized + 'rwlock> MapDeref for RwLockReadGuard<'rwlock, T> {
    fn map<X: 'static>(self, f: impl FnOnce(&Self::Target) -> &X) -> impl MapDeref<Target = X> {
        RwLockReadGuard::map(self, f)
    }
}

impl<'rwlock, T: ?Sized + 'rwlock> MapDeref for MappedRwLockReadGuard<'rwlock, T> {
    fn map<X: 'static>(self, f: impl FnOnce(&Self::Target) -> &X) -> impl MapDeref<Target = X> {
        MappedRwLockReadGuard::map(self, f)
    }
}
