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

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: SomeNeighbourhood<T>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference_key: StackCoeff,
    pub pattern_index: Option<usize>,

    /// does this harmony describe a valid tuning nof the current keys?
    pub valid: bool,
}

impl<T: IntervalBasis> Harmony<T> {
    pub fn new_dummy() -> Self {
        Self {
            neighbourhood: SomeNeighbourhood::Partial(Partial::new()),
            reference_key: 0,
            pattern_index: None {},
            valid: false,
        }
    }
}

pub struct HarmonyResult {
    pub finished: bool,
    pub progress: bool,
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
    fn start(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>);

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

    fn stop(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> HarmonyAdaptor<T, Self, Zero>;

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
