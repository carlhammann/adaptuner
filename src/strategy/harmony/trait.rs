use std::time::Instant;

use crate::{
    config::{ExtractConfig, IsHarmonyStrategyConfig},
    interval::stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    keystate::KeyState,
    msg::ToHarmony,
    neighbourhood::{Partial, SomeNeighbourhood},
    util::readerwriter::{Reader128, ReaderWriter},
};

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: SomeNeighbourhood<T>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference: StackCoeff,
    pub pattern_index: Option<usize>,
}

impl<T: IntervalBasis> Harmony<T> {
    pub fn new_dummy() -> Self {
        Self {
            neighbourhood: SomeNeighbourhood::Partial(Partial::new()),
            reference: 0,
            pattern_index: None {},
        }
    }
}

pub struct HarmonyResult {
    pub finished: bool,
    pub progress: bool,
}

pub trait HarmonyStrategy<T: StackType>: ExtractConfig<Self::Config> {
    type Config: IsHarmonyStrategyConfig<T, Realized = Self>;
    type Msg;

    fn new(config: Self::Config) -> Self;

    fn start(
        &mut self,
        time: Instant,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult;

    fn start_solve(
        &mut self,
        time: Instant,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult;

    fn step(
        &mut self,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult;

    fn stop(
        &mut self,
        time: Instant,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    );

    fn filter_to_harmony(msg: ToHarmony<T>) -> Option<Self::Msg>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> Option<Instant>;
}
