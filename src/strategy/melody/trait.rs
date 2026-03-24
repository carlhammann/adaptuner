use std::{
    ops::{Deref, DerefMut},
    time::Instant,
};

use crate::{
    config::IsMelodyStrategyConfig,
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::{FromStrategy, ToMelody},
    process::r#trait::StackWithTuning,
    strategy::harmony::r#trait::Harmony,
};

pub trait MelodyStrategyAdaptor<T: StackType>: {
    fn send(&self, msg: FromStrategy<T>) -> bool;
    fn key_states(&self) -> impl Deref<Target = [KeyState; 128]>;
    fn tunings(&self) -> impl DerefMut<Target = [StackWithTuning<T>; 128]>;
    fn harmony(&self) -> impl Deref<Target = Harmony<T>>;
}

pub trait MelodyStrategy<T: StackType, A: MelodyStrategyAdaptor<T>>: {
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

    /// Implementation of [ToMelody::SetTuningReference]
    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A);

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A);

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;
}
