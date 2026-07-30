use std::{marker::PhantomData, time::Instant};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, HarmonyLevel, KeyStateLevel, StrategyConfigLevel,
        TuningStateLevel,
    },
    bindable::BindableStrategyAction,
    config::IsHarmonyStrategyConfig,
    interval::stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    msg::ToHarmony,
    neighbourhood::{Partial, SomeNeighbourhood},
    process::r#trait::ProcessAdaptor,
    util::ordered_locks::{OrderedLocks, ReadAllowed, WriteAllowed, Zero},
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

pub type HarmonyAdaptor<T, S, P, L> = OrderedLocks<(HarmonyAdaptorTag, PhantomData<T>, S), P, L>;

pub trait HarmonyStrategy<T: StackType>: Sized {
    type Config: IsHarmonyStrategyConfig<T>;
    type Msg;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn start<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn start_solve<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [HarmonyStrategy::step]s are needed.
    fn step<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, P, Zero>);

    fn stop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> HarmonyAdaptor<T, Self, P, Zero>;

    /// deprecated for the same reason as [Strategy::reset]
    #[deprecated]
    fn reset<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> HarmonyAdaptor<T, Self, P, Zero>;

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    fn receive_msg<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        msg: Self::Msg,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, P, Zero>);

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    /// Should only do something if [StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, P, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, P, Zero>);
}
