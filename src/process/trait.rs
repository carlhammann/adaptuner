use std::{
    ops::DerefMut,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, RwLock,
    },
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
    util::mapderefmut::MapDerefMut,
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
    pub active_strategy_index: Arc<AtomicUsize>,
}

impl<T: StackType> Clone for ConcreteProcessAdaptor<T> {
    fn clone(&self) -> Self {
        Self {
            forward: self.forward.clone(),
            tunings: self.tunings.clone(),
            key_states: self.key_states.clone(),
            strategies: self.strategies.clone(),
            active_strategy_index: self.active_strategy_index.clone(),
        }
    }
}

/// The `Clone` implementation should make it so that the same underlying data is referenced.
pub trait ProcessAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromProcess<T>) -> bool;
    fn key_states(&self) -> impl DerefMut<Target = [KeyState; 128]>;
    fn tunings(&self) -> impl DerefMut<Target = [StackWithTuning<T>; 128]>;
    fn config(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>>;
    fn active_strategy_index(&self) -> usize;
    fn replace_active_strategy_index(&self, new_index: usize);
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
