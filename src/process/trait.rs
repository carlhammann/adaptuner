use std::sync::{mpsc, Arc};

use parking_lot::RwLock;

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, HarmonyLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel, TuningReferenceLevel, TuningStateLevel
    }, config::StrategyConfig, interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    }, keystate::KeyState, msg::FromProcess, reference::Reference, strategy::harmony::r#trait::Harmony, util::ordered_locks::{
        Access, AccessMut, IndexedAccess, IndexedAccessMut, ReadAllowed, WriteAllowed, impl_access, impl_access_mut, impl_indexed_access, impl_indexed_access_mut
    }
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub semitones: Semitones,
}

pub struct ProcessTag {}

impl ReadAllowed<KeyStateLevel> for ProcessTag{}
impl ReadAllowed<TuningStateLevel> for ProcessTag{}
impl ReadAllowed<StrategyConfigLevel> for ProcessTag{}
impl ReadAllowed<ActiveStrategyIndexLevel> for ProcessTag{}
impl ReadAllowed<TuningReferenceLevel> for ProcessTag{}
impl ReadAllowed<ReferenceLevel> for ProcessTag{}

impl WriteAllowed<KeyStateLevel> for ProcessTag {}
impl WriteAllowed<TuningStateLevel> for ProcessTag {}
impl WriteAllowed<ReferenceLevel> for ProcessTag {}

pub trait ProcessAdaptor:
    Sync
    + Send 
    + IndexedAccess<KeyStateLevel, usize, KeyState>
    + IndexedAccessMut<KeyStateLevel, usize, KeyState>
    + IndexedAccess<TuningStateLevel, usize, StackWithTuning<Self::StackType>>
    + IndexedAccessMut<TuningStateLevel, usize, StackWithTuning<Self::StackType>>
    + Access<StrategyConfigLevel, Vec<StrategyConfig<Self::StackType>>>
    // + AccessMut<StrategyConfigLevel, Vec<StrategyConfig<Self::StackType>>>
    + Access<ActiveStrategyIndexLevel, usize>
    // + AccessMut<ActiveStrategyIndexLevel, usize>
    + Access<TuningReferenceLevel, Reference<Self::StackType>>
    + Access<ReferenceLevel, Stack<Self::StackType>>
    + AccessMut<ReferenceLevel, Stack<Self::StackType>>
    + Access<HarmonyLevel, Option<Harmony<Self::StackType>>>
    + AccessMut<HarmonyLevel, Option<Harmony<Self::StackType>>>

{
    type StackType: StackType;
    fn send(&self, msg: FromProcess<Self::StackType>);
}

pub struct ConcreteProcessAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromProcess<T>>,
    pub key_states: [Arc<RwLock<KeyState>>; 128],
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub reference: Arc<RwLock<Stack<T>>>,
    pub tuning_reference: Arc<RwLock<Reference<T>>>,
    pub strategies: Arc<RwLock<Vec<StrategyConfig<T>>>>,
    pub active_strategy_index: Arc<RwLock<usize>>,
    pub harmony: Arc<RwLock<Option<Harmony<T>>>>,
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
            harmony: self.harmony.clone(),
        }
    }
}

impl_indexed_access! {<T:StackType>, ConcreteProcessAdaptor<T>, KeyStateLevel, usize, KeyState, |self, i| &self.key_states[i].read()}
impl_indexed_access_mut! {<T:StackType>, ConcreteProcessAdaptor<T>, KeyStateLevel, usize, KeyState, |self, i| &mut self.key_states[i].write()}
impl_indexed_access! {<T:StackType>, ConcreteProcessAdaptor<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &self.tunings[i].read()}
impl_indexed_access_mut! {<T:StackType>, ConcreteProcessAdaptor<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &mut self.tunings[i].write()}
impl_access! {<T:StackType>, ConcreteProcessAdaptor<T>, TuningReferenceLevel, Reference<T>, |self| &self.tuning_reference.read()}
impl_access! {<T:StackType>, ConcreteProcessAdaptor<T>, StrategyConfigLevel, Vec<StrategyConfig<T>>, |self| &self.strategies.read()}
impl_access! {<T:StackType>, ConcreteProcessAdaptor<T>, ActiveStrategyIndexLevel, usize, |self| &self.active_strategy_index.read()}
impl_access_mut! {<T:StackType>, ConcreteProcessAdaptor<T>, ActiveStrategyIndexLevel, usize, |self| &mut self.active_strategy_index.write()}
impl_access! {<T:StackType>, ConcreteProcessAdaptor<T>, ReferenceLevel, Stack<T>, |self| &self.reference.read()}
impl_access_mut! {<T:StackType>, ConcreteProcessAdaptor<T>, ReferenceLevel, Stack<T>, |self| &mut self.reference.write()}
impl_access! {<T:StackType>, ConcreteProcessAdaptor<T>, HarmonyLevel, Option<Harmony<T>>, |self| &self.harmony.read()}
impl_access_mut! {<T:StackType>, ConcreteProcessAdaptor<T>, HarmonyLevel, Option<Harmony<T>>, |self| &mut self.harmony.write()}

impl<T: StackType> ProcessAdaptor for ConcreteProcessAdaptor<T> {
    type StackType = T;
    #[inline]
    fn send(&self, msg: FromProcess<T>) {
        self.forward.send(msg);
    }
}
