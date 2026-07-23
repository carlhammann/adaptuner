use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    time::Instant,
};

use crate::{
    bindable::BindableStrategyAction, config::IsHarmonyStrategyConfig, interval::stacktype::r#trait::{IntervalBasis, StackCoeff, StackType}, keystate::KeyState, msg::ToHarmony, neighbourhood::{Partial, SomeNeighbourhood}
};

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: SomeNeighbourhood<T>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference: StackCoeff,
    pub pattern_index: Option<usize>,

    /// does this harmony describe a valid tuning nof the current keys?
    pub valid: bool,
}

impl<T: IntervalBasis> Harmony<T> {
    pub fn new_dummy() -> Self {
        Self {
            neighbourhood: SomeNeighbourhood::Partial(Partial::new()),
            reference: 0,
            pattern_index: None {},
            valid: false,
        }
    }
}

pub struct HarmonyResult {
    pub finished: bool,
    pub progress: bool,
}

/// Return value for the Function [HarmonyStrategyAdaptor::key_state_iter]
struct KeyStateIter<'a, T: StackType, A: HarmonyStrategyAdaptor<T>> {
    _phantom: PhantomData<T>,
    adaptor: &'a A,
    pos: usize,
}

impl<'a, T: StackType, A: HarmonyStrategyAdaptor<T>> Iterator for KeyStateIter<'a, T, A> {
    type Item = (usize, KeyState);
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= 128 {
            None {}
        } else {
            let res = (self.pos, *self.adaptor.key_state(self.pos));
            self.pos += 1;
            Some(res)
        }
    }
}

pub trait HarmonyStrategyAdaptor<T: StackType> {
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState>;
    fn key_state_iter(&self) -> impl Iterator<Item = (usize, KeyState)>
    where
        Self: Sized,
    {
        KeyStateIter {
            _phantom: PhantomData,
            adaptor: self,
            pos: 0,
        }
    }
    fn harmony(&self) -> impl DerefMut<Target = Harmony<T>>;
}

pub trait HarmonyStrategy<T: StackType, A: HarmonyStrategyAdaptor<T>> {
    type Config: IsHarmonyStrategyConfig<T>;
    type Msg;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn start(&mut self, time: Instant, adaptor: &A) -> HarmonyResult;

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn start_solve(&mut self, time: Instant, adaptor: &A) -> HarmonyResult;

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn step(&mut self, adaptor: &A) -> HarmonyResult;

    fn stop(&mut self, time: Instant, adaptor: &A);
    
    fn reset(&mut self, adaptor: &A);

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) -> Option<Instant>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    /// Should only do something if [StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: &A,
    ) -> Option<Instant>;
}
