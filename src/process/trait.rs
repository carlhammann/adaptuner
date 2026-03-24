use std::{
    ops::DerefMut,
    sync::{mpsc, Arc, MappedRwLockWriteGuard, RwLock, RwLockWriteGuard},
};

use crate::{
    config::StrategyConfig,
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    msg::FromProcess,
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub semitones: Semitones,
}

pub struct ConcreteProcessAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromProcess<T>>,
    pub tunings: Arc<RwLock<[StackWithTuning<T>; 128]>>,
    pub key_states: Arc<RwLock<[KeyState; 128]>>,
    pub strategies: Arc<RwLock<Vec<StrategyConfig<T>>>>,
}

impl<T: StackType> Clone for ConcreteProcessAdaptor<T> {
    fn clone(&self) -> Self {
        Self {
            forward: self.forward.clone(),
            tunings: self.tunings.clone(),
            key_states: self.key_states.clone(),
            strategies: self.strategies.clone(),
        }
    }
}

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

/// The `Clone` implementation should make it so that the same underlying data is referenced.
pub trait ProcessAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromProcess<T>) -> bool;
    fn key_states(&self) -> impl DerefMut<Target = [KeyState; 128]>;
    fn tunings(&self) -> impl DerefMut<Target = [StackWithTuning<T>; 128]>;
    fn config(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>>;
}

impl<T: StackType> ProcessAdaptor<T> for ConcreteProcessAdaptor<T> {
    #[inline]
    fn send(&self, msg: FromProcess<T>) -> bool {
        self.forward.send(msg).is_ok()
    }

    #[inline]
    fn key_states(&self) -> impl DerefMut<Target = [KeyState; 128]> {
        self.key_states.write().unwrap()
    }

    #[inline]
    fn tunings(&self) -> impl DerefMut<Target = [StackWithTuning<T>; 128]> {
        self.tunings.write().unwrap()
    }

    #[inline]
    fn config(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>> {
        self.strategies.write().unwrap()
    }
}
