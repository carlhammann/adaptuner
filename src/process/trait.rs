use std::{
    ops::DerefMut,
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
    util::mapderefmut::MapDerefMut,
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub semitones: Semitones,
}

pub struct ConcreteProcessAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromProcess<T>>,
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub key_states: [Arc<RwLock<KeyState>>; 128],
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
    /// index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> impl DerefMut<Target = KeyState>;
    /// index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
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
    fn key_state(&self, i:usize) -> impl DerefMut<Target = KeyState> {
        self.key_states[i].write()
    }

    #[inline]
    fn tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.tunings[i].write()
    }

    #[inline]
    fn config(&self) -> impl MapDerefMut<Target = Vec<StrategyConfig<T>>> {
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
