use std::{marker::PhantomData, time::Instant};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, HarmonyLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel,
        TuningReferenceLevel, TuningStateLevel,
    },
    bindable::BindableStrategyAction,
    config::IsMelodyStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{FromProcess, FromStrategy, ToMelody},
    process::r#trait::ProcessAdaptor,
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed, WriteAllowed, Zero},
};

pub struct MelodyAdaptorTag {}

impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<KeyStateLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<TuningStateLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<StrategyConfigLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<ActiveStrategyIndexLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<ReferenceLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<TuningReferenceLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<HarmonyLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}

impl<T: StackType, S: MelodyStrategy<T>> WriteAllowed<TuningStateLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}
impl<T: StackType, S: MelodyStrategy<T>> WriteAllowed<ReferenceLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}

pub type MelodyAdaptor<T, S, P, L> = OrderedLocks<(MelodyAdaptorTag, PhantomData<T>, S), P, L>;

impl<T: StackType, S: MelodyStrategy<T>, P: ProcessAdaptor<StackType = T>, L: Nat>
    MelodyAdaptor<T, S, P, L>
{
    pub fn send(&self, msg: FromStrategy<T>) {
        unsafe { self.inner() }.send(FromProcess::FromStrategy(msg));
    }
}

pub trait MelodyStrategy<T: StackType>: Sized {
    type Config: IsMelodyStrategyConfig<T>;

    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelody::TuneWithHarmony] and of [ToMelody::TuneNoHarmony], depending
    /// on the 'harmony_is_valid' argument.
    fn tune_with_harmony<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    /// Implementation of [ToMelody::Stop]
    fn stop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    /// Implementation of [ToMelody::Start].
    ///
    /// The 'with_harmony' argument schould be true iff the 'harmony' is already initialised.
    fn start<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    /// deprecated for the same reason as [Strategy::reset]
    #[deprecated]
    fn reset<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    /// Implementation of [ToMelody::SetTuningReference]
    fn update_tuning_reference<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    fn consider<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    fn receive_msg<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        msg: Self::Msg,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        action: &BindableStrategyAction,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, P, Zero>,
    ) -> MelodyAdaptor<T, Self, P, Zero>;
}
