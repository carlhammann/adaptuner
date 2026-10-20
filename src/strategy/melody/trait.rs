use std::marker::PhantomData;

use crate::time::Instant;

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::{
        lock_levels::{
            ActiveStrategyIndexLevel, AnchoringLevel, HarmonyLevel, KeyStateLevel, ReferenceLevel,
            StrategyConfigLevel, TuningReferenceLevel, TuningStateLevel,
        },
        ConcreteLocks,
    },
    bindable::BindableStrategyAction,
    config::IsMelodyStrategyConfig,
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromProcess, FromStrategy, ToMelody},
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed, WriteAllowed, Zero},
};

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum UndeterminedSpringAnchoringKind {
    LowestKey,
    HighestKey,
    Fundamental,
    Overtone,
    FundamentalOrOvertone,
    #[serde(rename_all = "kebab-case")]
    MostCommonNotes {
        min_number_of_common_notes:usize,
        count_pitch_classes: bool,
        fallback: Box<UndeterminedSpringAnchoringKind>,
    },
}

impl std::fmt::Display for UndeterminedSpringAnchoringKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UndeterminedSpringAnchoringKind::LowestKey => write!(f, "lowest key"),
            UndeterminedSpringAnchoringKind::HighestKey => write!(f, "highest key"),
            UndeterminedSpringAnchoringKind::Fundamental => write!(f, "highest fundamental"),
            UndeterminedSpringAnchoringKind::Overtone => write!(f, "lowest overtone"),
            UndeterminedSpringAnchoringKind::FundamentalOrOvertone => {
                write!(f, "fundamental or overtone, whichever is closer")
            }
            UndeterminedSpringAnchoringKind::MostCommonNotes {
                count_pitch_classes,
                fallback,
                min_number_of_common_notes
            } => {
                if *count_pitch_classes {
                    write!(f, "at least {min_number_of_common_notes} pitch classes from current scale or ")?;
                    fallback.fmt(f)
                } else {
                    write!(f, "at least {min_number_of_common_notes} notes from current scale or ")?;
                    fallback.fmt(f)
                }
            }
        }
    }
}

pub enum SpringAnchoringKind {
    LowestKey,
    HighestKey,
    Fundamental,
    Overtone,
    MostCommonNotes {
        count_pitch_classes: bool,
        number_of_hits: usize,
    },
}

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ChordAnchoringKind {
    LowestKey,
    HighestKey,
    ChordReference,
}

pub enum AnchoringKind {
    Spring(SpringAnchoringKind),
    Chord(ChordAnchoringKind),
    None,
}

pub struct Anchoring<T: IntervalBasis> {
    pub kind: AnchoringKind,
    pub key: StackCoeff,
    pub stack: Stack<T>,
}

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
impl<T: StackType, S: MelodyStrategy<T>> ReadAllowed<AnchoringLevel>
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
impl<T: StackType, S: MelodyStrategy<T>> WriteAllowed<AnchoringLevel>
    for (MelodyAdaptorTag, PhantomData<T>, S)
{
}

pub type MelodyAdaptor<T, S, L> =
    OrderedLocks<(MelodyAdaptorTag, PhantomData<T>, S), ConcreteLocks<T>, L>;

impl<T: StackType, S: MelodyStrategy<T>, L: Nat> MelodyAdaptor<T, S, L> {
    pub fn send(&self, msg: FromStrategy<T>) {
        let _ = unsafe { self.inner() }
            .from_process_tx
            .send(FromProcess::FromStrategy(msg));
    }
}

pub trait MelodyStrategy<T: StackType>: Sized {
    type Config: IsMelodyStrategyConfig<T>;

    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelody::TuneWithHarmony] and of [ToMelody::TuneNoHarmony], depending
    /// on the 'harmony_is_valid' argument.
    fn tune_with_harmony(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    /// Implementation of [ToMelody::Stop]
    fn stop(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    /// Implementation of [ToMelody::Start].
    ///
    /// The 'with_harmony' argument schould be true iff the 'harmony' is already initialised.
    fn start(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    /// Implementation of [ToMelody::SetTuningReference]
    fn update_tuning_reference(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    fn consider(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig::reacts_to_bound] returns true.
    fn handle_bound_action(
        &mut self,
        action: &BindableStrategyAction,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero>;
}
