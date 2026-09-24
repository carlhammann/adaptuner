use std::{marker::PhantomData, time::Instant};

use crate::{
    adaptors::{
        lock_levels::{
            ActiveStrategyIndexLevel, HarmonyLevel, KeyStateLevel, StrategyConfigLevel,
            TuningStateLevel,
        },
        ConcreteLocks,
    },
    bindable::BindableStrategyAction,
    config::IsHarmonyStrategyConfig,
    interval::stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    msg::ToHarmony,
    neighbourhood::{Partial, SomeNeighbourhood},
    util::ordered_locks::{OrderedLocks, ReadAllowed, WriteAllowed, Zero},
};

pub enum Harmony<T: IntervalBasis> {
    None,
    MatchedChord {
        neighbourhood: SomeNeighbourhood<T>,
        /// MIDI key number of the reference note, but may be outside the MIDI range
        reference_key: StackCoeff,
        /// MIDI key number of the lowest sounding key
        lowest_key: u8,
        /// MIDI key number of the highest sounding key
        highest_key: u8,
        pattern_index: usize,
    },
    SpringSolution {
        /// The intervals to the note played by the `lowest_key`. Will always contain a zero
        /// [Stack] for the zeroth entry, corresponding the the note of the `lowest_key`.
        neighbourhood: Partial<T>,
        lowest_key: u8,
        number_of_tries: u64,
        relaxed: bool,
    },
}

/// A description of the progress made determining a fit to the current harmony.
///
/// - finished means that no further attempts will be made by the harmony strategy.
/// - progress meand that the harmony strategy tried at least one attempt.
/// - perfect means that the solution is a perfect match: For chords, this means that all currently
///   sounding keys are matched, for springs this means that a just tuning was fond. perfect implies
///   finished, but not every finished result is perfect.
pub struct HarmonyResult {
    pub finished: bool,
    pub progress: bool,
    pub perfect: bool,
}

pub struct HarmonyAdaptorTag {}

impl<T: StackType, S: HarmonyStrategy<T>> ReadAllowed<KeyStateLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: HarmonyStrategy<T>> ReadAllowed<TuningStateLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: HarmonyStrategy<T>> ReadAllowed<StrategyConfigLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: HarmonyStrategy<T>> ReadAllowed<ActiveStrategyIndexLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: HarmonyStrategy<T>> ReadAllowed<HarmonyLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}

impl<T: StackType, S: HarmonyStrategy<T>> WriteAllowed<HarmonyLevel>
    for (HarmonyAdaptorTag, PhantomData<T>, S)
{
}

pub type HarmonyAdaptor<T, S, L> =
    OrderedLocks<(HarmonyAdaptorTag, PhantomData<T>, S), ConcreteLocks<T>, L>;

pub trait HarmonyStrategy<T: StackType>: Sized {
    type Config: IsHarmonyStrategyConfig<T>;
    type Msg;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn start_solve(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>);

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn step(
        &mut self,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>);

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>);

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    /// Should only do something if [crate::config::StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>);
}
