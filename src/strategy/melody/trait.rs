use std::{
    ops::{Deref, DerefMut},
    time::Instant,
};

use crate::{
    bindable::BindableStrategyAction,
    config::IsMelodyStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromStrategy, ToMelody},
    process::r#trait::StackWithTuning,
    reference::Reference,
    strategy::harmony::r#trait::Harmony,
};

/// [key_state], [tuning], and [tuning_reference] must be locked in that order.
pub trait MelodyStrategyAdaptor<T: StackType> {
    fn send(&self, msg: FromStrategy<T>) -> bool;
    /// index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState>;
    /// index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
    fn tuning_mut(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
    fn reference(&self) -> impl Deref<Target = Stack<T>>;
    fn reference_mut(&self) -> impl DerefMut<Target = Stack<T>>;
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>>;
    fn harmony(&self) -> impl Deref<Target = Harmony<T>>;
}

pub trait MelodyStrategy<T: StackType, A: MelodyStrategyAdaptor<T>> {
    type Config: IsMelodyStrategyConfig<T>;

    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelody::TuneWithHarmony] and of [ToMelody::TuneNoHarmony], depending
    /// on the 'harmony_is_valid' argument.
    fn tune_with_harmony(&mut self, time: Instant, adaptor: &A);

    /// Implementation of [ToMelody::Stop]
    fn stop(&mut self, time: Instant, adaptor: &A);

    /// Implementation of [ToMelody::Start].
    ///
    /// The 'with_harmony' argument schould be true iff the 'harmony' is already initialised.
    fn start(&mut self, time: Instant, adaptor: &A);
    
    fn reset(&mut self, adaptor: &A);

    /// Implementation of [ToMelody::SetTuningReference]
    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A);

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A);

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action(&mut self, action: &BindableStrategyAction, time: Instant, adaptor: &A);
}
