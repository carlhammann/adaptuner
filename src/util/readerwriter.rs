use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

pub struct Reader<X>(Arc<RwLock<X>>);

impl<X> Reader<X> {
    pub fn new(x: Arc<RwLock<X>>) -> Self {
        Reader(x)
    }

    pub fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let Reader(x) = self;
        x.read().unwrap()
    }
}

pub struct ReaderWriter<X>(Arc<RwLock<X>>);

impl<X> ReaderWriter<X> {
    pub fn new(x: Arc<RwLock<X>>) -> Self {
        ReaderWriter(x)
    }

    pub fn write(&self) -> impl DerefMut<Target = X> + use<'_, X> {
        let ReaderWriter(x) = self;
        x.write().unwrap()
    }

    pub fn read(&self) -> impl Deref<Target = X> + use<'_, X> {
        let ReaderWriter(x) = self;
        x.read().unwrap()
    }
}
