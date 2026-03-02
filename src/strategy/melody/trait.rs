use std::time::Instant;

use crate::{
    config::IsMelodyStrategyConfig,
    interval::stacktype::r#trait::StackType,
    msg::ToMelody,
    reference::Reference,
    strategy::{harmony::r#trait::Harmony, r#trait::StrategyAdaptor},
    util::readerwriter::Reader,
};

pub trait MelodyStrategy<T: StackType> {
    type Config: IsMelodyStrategyConfig<T, Realized = Self>;

    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelody::TuneWithHarmony] and of [ToMelody::TuneNoHarmony], depending
    /// on the 'harmony_is_valid' argument.
    fn tune_with_harmony(
        &mut self,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    );

    /// Implementation of [ToMelody::Stop]
    fn stop(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>);

    /// Implementation of [ToMelody::Start].
    ///
    /// The 'with_harmony' argument schould be true iff the 'harmony' is already initialised.
    fn start(
        &mut self,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    );

    /// Implementation of [ToMelody::SetTuningReference]
    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    );

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    );

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;
}
