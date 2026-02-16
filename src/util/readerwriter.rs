use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

pub struct Reader<X>(Arc<RwLock<X>>);

impl<X> Clone for Reader<X> {
    fn clone(&self) -> Self {
        let Reader(x) = self;
        Reader(x.clone())
    }
}

impl<X> Reader<X> {
    pub fn new(x: X) -> Self {
        Reader(Arc::new(RwLock::new(x)))
    }

    pub fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let Reader(x) = self;
        x.read().unwrap()
    }
}

pub struct ReaderWriter<X>(Arc<RwLock<X>>);

impl<X> Clone for ReaderWriter<X> {
    fn clone(&self) -> Self {
        let ReaderWriter(x) = self;
        ReaderWriter(x.clone())
    }
}

impl<X> ReaderWriter<X> {
    pub fn new(x: X) -> Self {
        ReaderWriter(Arc::new(RwLock::new(x)))
    }

    pub fn write(&self) -> impl DerefMut<Target = X> + use<'_, X> {
        let ReaderWriter(x) = self;
        x.write().unwrap()
    }

    pub fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let ReaderWriter(x) = self;
        x.read().unwrap()
    }

    pub fn into_reader(self) -> Reader<X> {
        let ReaderWriter(x) = self;
        Reader(x)
    }
}
