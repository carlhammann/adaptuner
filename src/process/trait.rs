use std::{
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use parking_lot::RwLock;

use crate::{
    config::StrategyConfig,
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    msg::FromProcess,
    reference::Reference,
    util::{mapderef::MapDeref, mapderefmut::MapDerefMut},
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub semitones: Semitones,
}

/// [key_states], [tunings], and [tuning_reference] must be locked and unlocked in that order.
pub struct ConcreteProcessAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromProcess<T>>,
    pub key_states: [Arc<RwLock<KeyState>>; 128],
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub reference: Arc<RwLock<Stack<T>>>,
    pub tuning_reference: Arc<RwLock<Reference<T>>>,
    pub strategies: Arc<RwLock<Vec<StrategyConfig<T>>>>,
    pub active_strategy_index: Arc<AtomicUsize>,
}

impl<T: StackType> Clone for ConcreteProcessAdaptor<T> {
    fn clone(&self) -> Self {
        Self {
            forward: self.forward.clone(),
            key_states: self.key_states.clone(),
            tunings: self.tunings.clone(),
            reference: self.reference.clone(),
            tuning_reference: self.tuning_reference.clone(),
            strategies: self.strategies.clone(),
            active_strategy_index: self.active_strategy_index.clone(),
        }
    }
}

/// The `Clone` implementation should make it so that the same underlying data is referenced.
pub trait ProcessAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromProcess<T>) -> bool;
    /// index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> impl DerefMut<Target = KeyState>;
    /// index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
    fn tuning_mut(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
    fn reference(&self) -> impl Deref<Target = Stack<T>>;
    fn reference_mut(&self) -> impl DerefMut<Target = Stack<T>>;
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>>;
    fn strategy_config(&self) -> impl MapDeref<Target = Vec<StrategyConfig<T>>>;
    fn strategy_config_mut(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>>;
    fn active_strategy_index(&self) -> usize;
    fn replace_active_strategy_index(&self, new_index: usize);
}

impl<T: StackType> ProcessAdaptor<T> for ConcreteProcessAdaptor<T> {
    #[inline]
    fn send(&self, msg: FromProcess<T>) -> bool {
        self.forward.send(msg).is_ok()
    }

    #[inline]
    fn key_state(&self, i: usize) -> impl DerefMut<Target = KeyState> {
        self.key_states[i].write()
    }

    #[inline]
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings[i].read()
    }

    #[inline]
    fn tuning_mut(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.tunings[i].write()
    }

    #[inline]
    fn reference(&self) -> impl Deref<Target = Stack<T>> {
        self.reference.read()
    }

    #[inline]
    fn reference_mut(&self) -> impl DerefMut<Target = Stack<T>> {
        self.reference.write()
    }

    #[inline]
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>> {
        self.tuning_reference.read()
    }

    #[inline]
    fn strategy_config(&self) -> impl MapDeref<Target = Vec<StrategyConfig<T>>> {
        self.strategies.read()
    }

    #[inline]
    fn strategy_config_mut(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>> {
        self.strategies.write()
    }

    #[inline]
    fn active_strategy_index(&self) -> usize {
        self.active_strategy_index.load(Ordering::Acquire)
    }

    #[inline]
    fn replace_active_strategy_index(&self, new_index: usize) {
        self.active_strategy_index
            .store(new_index, Ordering::Release);
    }
}
