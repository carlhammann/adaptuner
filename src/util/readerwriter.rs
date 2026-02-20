use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub trait Reader<X>: Clone {
    fn read(&self) -> impl Deref<Target = X>;
}

pub trait ReaderWriter<X>: Reader<X> {
    fn write(&self) -> impl DerefMut<Target = X>;
}

pub struct ConcreteReaderWriter<X>(Arc<RwLock<X>>);

impl<X> ConcreteReaderWriter<X> {
    pub fn new(x: X) -> Self {
        Self(Arc::new(RwLock::new(x)))
    }
}

impl<X> Clone for ConcreteReaderWriter<X> {
    fn clone(&self) -> Self {
        let Self(x) = self;
        Self(x.clone())
    }
}

impl<X> Reader<X> for ConcreteReaderWriter<X> {
    fn read(&self) -> impl Deref<Target = X> {
        let Self(x) = self;
        x.read().unwrap()
    }
}

impl<X> ReaderWriter<X> for ConcreteReaderWriter<X> {
    fn write(&self) -> impl DerefMut<Target = X> {
        let Self(x) = self;
        x.write().unwrap()
    }
}

pub trait Reader128<X>: Clone {
    fn read(&self, i: usize) -> impl Deref<Target = X>;
    fn read_all(&self) -> impl Deref<Target = [X; 128]>;
}

pub trait ReaderWriter128<X>: Reader128<X> {
    fn write(&self, i: usize) -> impl DerefMut<Target = X>;
    fn write_all(&self) -> impl DerefMut<Target = [X; 128]>;
}

pub struct ConcreteReader128<X>(Arc<RwLock<[X; 128]>>);

impl<X> Clone for ConcreteReader128<X> {
    fn clone(&self) -> Self {
        let Self(x) = self;
        Self(x.clone())
    }
}

impl<X> ConcreteReader128<X> {
    pub fn new(x: [X; 128]) -> Self {
        Self(Arc::new(RwLock::new(x)))
    }
}

impl<X> Reader128<X> for ConcreteReader128<X> {
    fn read(&self, i: usize) -> impl Deref<Target = X> {
        let Self(x) = self;
        RwLockReadGuard::map(x.read().unwrap(), |v: &[X; 128]| &v[i])
    }

    fn read_all(&self) -> impl Deref<Target = [X; 128]> {
        let Self(x) = self;
        x.read().unwrap()
    }
}

pub struct ConcreteReaderWriter128<X>(Arc<RwLock<[X; 128]>>);

impl<X> Clone for ConcreteReaderWriter128<X> {
    fn clone(&self) -> Self {
        let Self(x) = self;
        Self(x.clone())
    }
}

impl<X> ConcreteReaderWriter128<X> {
    pub fn new(x: [X; 128]) -> Self {
        Self(Arc::new(RwLock::new(x)))
    }

    pub fn into_reader(self) -> ConcreteReader128<X> {
        let Self(x) = self;
        ConcreteReader128(x)
    }
}

impl<X> Reader128<X> for ConcreteReaderWriter128<X> {
    fn read(&self, i: usize) -> impl Deref<Target = X> {
        let Self(x) = self;
        RwLockReadGuard::map(x.read().unwrap(), |v: &[X; 128]| &v[i])
    }

    fn read_all(&self) -> impl Deref<Target = [X; 128]> {
        let Self(x) = self;
        x.read().unwrap()
    }
}

impl<X> ReaderWriter128<X> for ConcreteReaderWriter128<X> {
    fn write(&self, i: usize) -> impl DerefMut<Target = X> {
        let Self(x) = self;
        RwLockWriteGuard::map(x.write().unwrap(), |v: &mut [X; 128]| &mut v[i])
    }

    fn write_all(&self) -> impl DerefMut<Target = [X; 128]> {
        let Self(x) = self;
        x.write().unwrap()
    }
}
