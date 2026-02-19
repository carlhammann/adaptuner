use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

pub trait Reader<X>: Clone {
    fn read(&self) -> impl Deref<Target = X>;
}

pub trait ReaderWriter<X>: Reader<X> {
    fn write(&self) -> impl DerefMut<Target = X>;
}

pub struct ConcreteReader<X>(Arc<RwLock<X>>);

impl<X> Clone for ConcreteReader<X> {
    fn clone(&self) -> Self {
        let ConcreteReader(x) = self;
        ConcreteReader(x.clone())
    }
}

impl<X> ConcreteReader<X> {
    pub fn new(x: X) -> Self {
        ConcreteReader(Arc::new(RwLock::new(x)))
    }
}

impl<X> Reader<X> for ConcreteReader<X> {
    fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let ConcreteReader(x) = self;
        x.read().unwrap()
    }
}

pub struct ConcreteReaderWriter<X>(Arc<RwLock<X>>);

impl<X> Clone for ConcreteReaderWriter<X> {
    fn clone(&self) -> Self {
        let ConcreteReaderWriter(x) = self;
        ConcreteReaderWriter(x.clone())
    }
}

impl<X> ConcreteReaderWriter<X> {
    pub fn new(x: X) -> Self {
        ConcreteReaderWriter(Arc::new(RwLock::new(x)))
    }

    pub fn into_reader(self) -> ConcreteReader<X> {
        let ConcreteReaderWriter(x) = self;
        ConcreteReader(x)
    }
}

impl<X> Reader<X> for ConcreteReaderWriter<X> {
    fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let ConcreteReaderWriter(x) = self;
        x.read().unwrap()
    }
}

impl<X> ReaderWriter<X> for ConcreteReaderWriter<X> {
    fn write(&self) -> impl DerefMut<Target = X> + use<'_, X> {
        let ConcreteReaderWriter(x) = self;
        x.write().unwrap()
    }
}
