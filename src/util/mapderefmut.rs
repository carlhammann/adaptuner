use std::ops::DerefMut;

use parking_lot::{MappedRwLockWriteGuard, RwLockWriteGuard};

pub trait MapDerefMut: DerefMut {
    fn map<X: 'static>(
        self,
        f: impl FnOnce(&mut Self::Target) -> &mut X,
    ) -> impl MapDerefMut<Target = X>;
}

impl<'rwlock, T: ?Sized + 'rwlock> MapDerefMut for RwLockWriteGuard<'rwlock, T> {
    fn map<X: 'static>(
        self,
        f: impl FnOnce(&mut Self::Target) -> &mut X,
    ) -> impl MapDerefMut<Target = X> {
        RwLockWriteGuard::map(self, f)
    }
}

impl<'rwlock, T: ?Sized + 'rwlock> MapDerefMut for MappedRwLockWriteGuard<'rwlock, T> {
    fn map<X: 'static>(
        self,
        f: impl FnOnce(&mut Self::Target) -> &mut X,
    ) -> impl MapDerefMut<Target = X> {
        MappedRwLockWriteGuard::map(self, f)
    }
}
