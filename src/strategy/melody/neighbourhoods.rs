use std::time::Instant;

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, AnchoringLevel, HarmonyLevel, KeyStateLevel, ReferenceLevel,
        StrategyConfigLevel,
    },
    bindable::BindableStrategyAction,
    config::{IsMelodyStrategyConfig, MelodyStrategyConfig, Named, StrategyConfig},
    interval::{
        fundamental::{fundamental_or_overtone, HasFundamental, HasOvertone},
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToMelody, ToStaticNeighbourhoodsAsMelody},
    neighbourhood::{
        CompleteNeighbourhood, Neighbourhood, Partial, SomeCompleteNeighbourhood, SomeNeighbourhood,
    },
    strategy::{
        harmony::r#trait::Harmony,
        melody::r#trait::{
            Anchoring, AnchoringKind, ChordAnchoringKind, MelodyAdaptor, MelodyStrategy,
            SpringAnchoringKind, UndeterminedSpringAnchoringKind,
        },
    },
    util::ordered_locks::{AtMost, Succ, Zero},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    /// This Vec must never be empty
    pub scales: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub initial_reference: Stack<T>,
    pub spring_anchoring_kind: UndeterminedSpringAnchoringKind,
    pub chord_anchoring_kind: ChordAnchoringKind,
}

pub struct StaticNeighbourhoodsAsMelody<T: StackType> {
    /// This Vec must never be empty
    scales: Vec<SomeCompleteNeighbourhood<T>>,
    curr_scale_index: usize,

    tmp_stack: Stack<T>,
}

impl<T: StackType> IsMelodyStrategyConfig<T> for StaticNeighbourhoodsAsMelodyConfig<T> {
    fn initial_scale_reference(&self) -> Option<&Stack<T>> {
        Some(&self.initial_reference)
    }
}

impl<T: StackType + HasOvertone + HasFundamental> StaticNeighbourhoodsAsMelody<T> {
    fn tune_without_harmony<L>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<KeyStateLevel>,
    {
        adaptor.send(FromStrategy::UpdateHarmony);
        adaptor.for_all_sounding_tunings_mut(|i, the_tuning, mut adaptor| {
            self.tmp_stack.clone_from(&the_tuning.stack);
            (_, adaptor) = adaptor.reference(|reference, _| {
                self.scales[self.curr_scale_index].write_absolute_stack(
                    &mut the_tuning.stack,
                    i as StackCoeff,
                    reference,
                )
            });

            let mut retune = self.tmp_stack != the_tuning.stack;
            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let new_semitones = the_tuning.stack.absolute_semitones(c4_semitones);
            if new_semitones != the_tuning.semitones {
                the_tuning.semitones = new_semitones;
                retune = true;
            }
            if retune {
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        })
    }

    fn tune_with_spring_harmony<L>(
        &mut self,
        time: Instant,
        neighbourhood: &Partial<T>,
        lowest_key: u8,
        mut adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<KeyStateLevel>
            + AtMost<ReferenceLevel>
            + AtMost<StrategyConfigLevel>
            + AtMost<AnchoringLevel>,
    {
        let undetermined_anchoring_kind;
        (undetermined_anchoring_kind, adaptor) =
            adaptor.config(|conf, _| conf.spring_anchoring_kind);
        let harmony_reference_offset;
        let harmony_reference_key;
        let anchoring_kind;
        match undetermined_anchoring_kind {
            UndeterminedSpringAnchoringKind::Overtone => {
                harmony_reference_offset = T::overtone_many(neighbourhood.iter().map(|x| x.1));
                harmony_reference_key =
                    lowest_key as StackCoeff + harmony_reference_offset.key_distance();
                anchoring_kind = SpringAnchoringKind::Overtone;
            }
            UndeterminedSpringAnchoringKind::Fundamental => {
                harmony_reference_offset = T::fundamental_many(neighbourhood.iter().map(|x| x.1));
                harmony_reference_key =
                    lowest_key as StackCoeff + harmony_reference_offset.key_distance();
                anchoring_kind = SpringAnchoringKind::Fundamental;
            }
            UndeterminedSpringAnchoringKind::FundamentalOrOvertone => {
                let is_utonal;
                (is_utonal, harmony_reference_offset) = fundamental_or_overtone(neighbourhood);
                harmony_reference_key =
                    lowest_key as StackCoeff + harmony_reference_offset.key_distance();
                if is_utonal {
                    anchoring_kind = SpringAnchoringKind::Fundamental;
                } else {
                    anchoring_kind = SpringAnchoringKind::Overtone;
                }
            }
            UndeterminedSpringAnchoringKind::LowestKey => {
                // This uses the fact that the 'neighbourhood' argument comes from a
                // [Harmony::SprigSolution], and therefore contains the zero stack as the lowest
                // entry.
                harmony_reference_offset = Stack::new_zero();
                harmony_reference_key = lowest_key as StackCoeff;
                anchoring_kind = SpringAnchoringKind::LowestKey;
            }
            UndeterminedSpringAnchoringKind::HighestKey => {
                if let Some((k, s)) = neighbourhood.highest() {
                    harmony_reference_key = *k + lowest_key as StackCoeff;
                    harmony_reference_offset = s.clone();
                    anchoring_kind = SpringAnchoringKind::HighestKey;
                } else {
                    panic!("tune_with_spring_harmony got an empty neighbourhood from a spring solution")
                }
            }
        }
        // let (_, reference_offset_stack) = fundamental_or_overtone(neighbourhood);
        let harmony_reference_stack;
        (harmony_reference_stack, adaptor) = adaptor.reference(|adaptor_reference, _| {
            self.scales[self.curr_scale_index]
                .get_absolute_stack(harmony_reference_key, adaptor_reference)
        });

        adaptor = adaptor.for_all_sounding_tunings_mut(|i, the_tuning, mut adaptor| {
            self.tmp_stack.clone_from(&the_tuning.stack);
            if neighbourhood.try_write_relative_stack(
                &mut the_tuning.stack,
                i as StackCoeff - lowest_key as StackCoeff,
            ) {
                the_tuning.stack.scaled_add(1, &harmony_reference_stack);
                the_tuning.stack.scaled_add(-1, &harmony_reference_offset)
            } else {
                (_, adaptor) = adaptor.reference(|adaptor_reference, _| {
                    self.scales[self.curr_scale_index].write_absolute_stack(
                        &mut the_tuning.stack,
                        i as StackCoeff,
                        adaptor_reference,
                    )
                });
            }

            let mut retune = self.tmp_stack != the_tuning.stack;
            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let new_semitones = the_tuning.stack.absolute_semitones(c4_semitones);
            if new_semitones != the_tuning.semitones {
                the_tuning.semitones = new_semitones;
                retune = true;
            }
            if retune {
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        });
        (_, adaptor) = adaptor.anchoring_mut(|a, _| {
            *a = Anchoring {
                kind: AnchoringKind::Spring(anchoring_kind),
                key: harmony_reference_key,
                stack: harmony_reference_stack,
            }
        });
        adaptor.send(FromStrategy::UpdateHarmony);
        adaptor
    }

    fn tune_with_matched_chord<L>(
        &mut self,
        time: Instant,
        neighbourhood: &SomeNeighbourhood<T>,
        reference_key: StackCoeff,
        lowest_key: u8,
        highest_key: u8,
        mut adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<KeyStateLevel>
            + AtMost<ReferenceLevel>
            + AtMost<AnchoringLevel>
            + AtMost<StrategyConfigLevel>,
    {
        let anchoring_kind;
        (anchoring_kind, adaptor) = adaptor.config(|conf, _| conf.chord_anchoring_kind);
        let harmony_reference_key;
        let mut harmony_reference_offset = Stack::new_zero();
        match anchoring_kind {
            ChordAnchoringKind::ChordReference => {
                harmony_reference_key = reference_key;
            }
            ChordAnchoringKind::LowestKey => {
                harmony_reference_key = lowest_key as StackCoeff;
                neighbourhood.try_write_relative_stack(
                    &mut harmony_reference_offset,
                    harmony_reference_key - reference_key,
                );
            }
            ChordAnchoringKind::HighestKey => {
                harmony_reference_key = highest_key as StackCoeff;
                neighbourhood.try_write_relative_stack(
                    &mut harmony_reference_offset,
                    harmony_reference_key - reference_key,
                );
            }
        }
        let harmony_reference_stack;
        (harmony_reference_stack, adaptor) = adaptor.reference(|adaptor_reference, _| {
            self.scales[self.curr_scale_index]
                .get_absolute_stack(harmony_reference_key, adaptor_reference)
        });
        adaptor = adaptor.for_all_sounding_tunings_mut(|i, the_tuning, mut adaptor| {
            self.tmp_stack.clone_from(&the_tuning.stack);
            if neighbourhood
                .try_write_relative_stack(&mut the_tuning.stack, i as StackCoeff - reference_key)
            {
                the_tuning.stack.scaled_add(1, &harmony_reference_stack);
                the_tuning.stack.scaled_add(-1, &harmony_reference_offset)
            } else {
                (_, adaptor) = adaptor.reference(|adaptor_reference, _| {
                    self.scales[self.curr_scale_index].write_absolute_stack(
                        &mut the_tuning.stack,
                        i as StackCoeff,
                        adaptor_reference,
                    )
                });
            }

            let mut retune = self.tmp_stack != the_tuning.stack;
            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let new_semitones = the_tuning.stack.absolute_semitones(c4_semitones);
            if new_semitones != the_tuning.semitones {
                the_tuning.semitones = new_semitones;
                retune = true;
            }
            if retune {
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        });
        (_, adaptor) = adaptor.anchoring_mut(|a, _| {
            *a = Anchoring {
                kind: AnchoringKind::Chord(anchoring_kind),
                key: harmony_reference_key,
                stack: harmony_reference_stack,
            }
        });
        adaptor.send(FromStrategy::UpdateHarmony);
        adaptor
    }

    fn update_all_tunings_and_send<L>(
        &mut self,
        time: Instant,
        mut adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<HarmonyLevel>, // for the called sub-methods: + AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        (_, adaptor) = adaptor.harmony(|harmony, adaptor| match harmony {
            Harmony::SpringSolution {
                neighbourhood,
                lowest_key,
                ..
            } => self.tune_with_spring_harmony(time, &neighbourhood, *lowest_key, adaptor),
            Harmony::MatchedChord {
                neighbourhood,
                reference_key,
                lowest_key,
                highest_key,
                ..
            } => self.tune_with_matched_chord(
                time,
                &neighbourhood,
                *reference_key,
                *lowest_key,
                *highest_key,
                adaptor,
            ),
            Harmony::None => self.tune_without_harmony(time, adaptor),
        });
        adaptor
    }

    /// returns true iff the reference changed
    fn set_reference<L: AtMost<ReferenceLevel>>(
        &mut self,
        new_reference: Stack<T>,
        adaptor: MelodyAdaptor<T, Self, L>,
    ) -> (bool, MelodyAdaptor<T, Self, L>) {
        adaptor.reference_mut(|reference, adaptor| {
            if new_reference != *reference {
                reference.clone_from(&new_reference);
                adaptor.send(FromStrategy::UpdateReference {});
                true
            } else {
                false
            }
        })
    }

    /// returns true iff the reference changed
    fn set_reference_to_current<L>(
        &mut self,
        adaptor: MelodyAdaptor<T, Self, L>,
    ) -> (bool, MelodyAdaptor<T, Self, L>)
    where
        L: AtMost<HarmonyLevel>, // + AtMost<ReferenceLevel>
    {
        adaptor.harmony(|harmony, mut adaptor| {
            let harmony_reference_key = match harmony {
                Harmony::MatchedChord {
                    reference_key,
                    lowest_key,
                    highest_key,
                    ..
                } => {
                    let k;
                    (k, adaptor) = adaptor.config(|conf, _| match conf.chord_anchoring_kind {
                        ChordAnchoringKind::HighestKey => Some(*highest_key as StackCoeff),
                        ChordAnchoringKind::LowestKey => Some(*lowest_key as StackCoeff),
                        ChordAnchoringKind::ChordReference => Some(*reference_key),
                    });
                    k
                }
                Harmony::SpringSolution {
                    lowest_key,
                    neighbourhood,
                    ..
                } => {
                    let k;
                    (k, adaptor) = adaptor.config(|conf, _| match conf.spring_anchoring_kind {
                        UndeterminedSpringAnchoringKind::HighestKey => {
                            if let Some((h, _)) = neighbourhood.highest() {
                                Some(*h)
                            } else {
                                panic!(
                                    "set_reference_to_current encountered empty \
                                    neighbourhood for a spring solution!"
                                )
                            }
                        }
                        UndeterminedSpringAnchoringKind::LowestKey => {
                            Some(*lowest_key as StackCoeff)
                        }
                        UndeterminedSpringAnchoringKind::Fundamental => {
                            let reference_offset_stack =
                                T::fundamental_many(neighbourhood.iter().map(|x| x.1));
                            let reference_key =
                                *lowest_key as StackCoeff + reference_offset_stack.key_distance();
                            Some(reference_key)
                        }
                        UndeterminedSpringAnchoringKind::Overtone => {
                            let reference_offset_stack =
                                T::overtone_many(neighbourhood.iter().map(|x| x.1));
                            let reference_key =
                                *lowest_key as StackCoeff + reference_offset_stack.key_distance();
                            Some(reference_key)
                        }
                        UndeterminedSpringAnchoringKind::FundamentalOrOvertone => {
                            let (_, reference_offset_stack) =
                                fundamental_or_overtone(&neighbourhood);
                            let reference_key =
                                *lowest_key as StackCoeff + reference_offset_stack.key_distance();
                            Some(reference_key)
                        }
                    });
                    k
                }
                Harmony::None => None {},
            };

            if let Some(reference_key) = harmony_reference_key {
                adaptor
                    .reference_mut(|adaptor_reference, adaptor| {
                        self.scales[self.curr_scale_index].write_absolute_stack(
                            &mut self.tmp_stack,
                            reference_key,
                            adaptor_reference,
                        );

                        if *adaptor_reference != self.tmp_stack {
                            adaptor_reference.clone_from(&self.tmp_stack);
                            adaptor.send(FromStrategy::UpdateReference {});
                            true
                        } else {
                            false
                        }
                    })
                    .0
            } else {
                false
            }
        })
    }

    /// returns true iff the reference changed
    fn set_reference_to_extreme<L>(
        &mut self,
        to_highest: bool,
        mut adaptor: MelodyAdaptor<T, Self, L>,
    ) -> (bool, MelodyAdaptor<T, Self, L>)
    where
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        (_, adaptor) = adaptor.reference(|r, _| self.tmp_stack.clone_from(r));

        if to_highest {
            for i in (0..128).rev() {
                let mut found = false;
                (_, adaptor) = adaptor.key_state(i, |k, adaptor| {
                    if k.is_sounding() {
                        adaptor.tuning(i, |t, _| self.tmp_stack.clone_from(&t.stack));
                        found = true;
                    }
                });
                if found {
                    break;
                }
            }
        } else {
            for i in 0..128 {
                let mut found = false;
                (_, adaptor) = adaptor.key_state(i, |k, adaptor| {
                    if k.is_sounding() {
                        adaptor.tuning(i, |t, _| self.tmp_stack.clone_from(&t.stack));
                        found = true;
                    }
                });
                if found {
                    break;
                }
            }
        }

        adaptor.reference_mut(|old_reference, adaptor| {
            if *old_reference != self.tmp_stack {
                old_reference.clone_from(&self.tmp_stack);
                adaptor.send(FromStrategy::UpdateReference {});
                true
            } else {
                false
            }
        })
    }

    fn reset_scale_and_tunings(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        adaptor.send(FromStrategy::SelectScale {
            index: self.curr_scale_index,
        });
        self.scales[self.curr_scale_index].for_each_stack(|_, stack| {
            let _ = adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });
        self.tune_with_harmony(time, adaptor)
    }
}

impl<T: StackType + HasFundamental + HasOvertone, L: AtMost<StrategyConfigLevel>>
    MelodyAdaptor<T, StaticNeighbourhoodsAsMelody<T>, L>
{
    fn config<R>(
        self,
        mut f: impl FnMut(
            &StaticNeighbourhoodsAsMelodyConfig<T>,
            MelodyAdaptor<T, StaticNeighbourhoodsAsMelody<T>, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self) {
        self.active_strategy(|conf, adaptor| match conf {
            StrategyConfig::TwoStep {
                melody: MelodyStrategyConfig::StaticNeighbourhoods(config),
                ..
            } => f(config, adaptor),
            _ => panic!(
                "Wrong type of melody strategy config: expected StaticNeighbourhoodsAsMelodyConfig"
            ),
        })
    }
}

impl<T: StackType + HasOvertone + HasFundamental> MelodyStrategy<T>
    for StaticNeighbourhoodsAsMelody<T>
{
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(mut config: Self::Config) -> Self {
        Self {
            scales: config.scales.drain(..).map(|n| n.named).collect(),
            curr_scale_index: 0,
            tmp_stack: Stack::new_zero(),
        }
    }

    fn tune_with_harmony(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        self.update_all_tunings_and_send(time, adaptor)
    }

    fn stop(
        &mut self,
        _time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        adaptor
    }

    fn start(
        &mut self,
        time: Instant,
        mut adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        (_, adaptor) = adaptor.initial_scale_reference(|m_initial_reference, adaptor| {
            if let Some(initial_reference) = m_initial_reference {
                adaptor.send(FromStrategy::UpdateReference {});
                adaptor.reference_mut(|reference, _| {
                    reference.clone_from(initial_reference);
                });
            }
        });

        self.reset_scale_and_tunings(time, adaptor)
    }

    fn update_tuning_reference(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        adaptor.for_all_sounding_tunings_mut(|i, the_tuning, mut adaptor| {
            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let new_semitones = the_tuning.stack.absolute_semitones(c4_semitones);
            the_tuning.semitones = new_semitones;
            adaptor.send(FromStrategy::Retune {
                note: i as u8,
                time,
            });
        })
    }

    fn consider(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        let inserted_stack = self.scales[self.curr_scale_index].insert(&stack).clone();
        let _ = adaptor.send(FromStrategy::Consider {
            stack: inserted_stack,
        });
        self.update_all_tunings_and_send(time, adaptor)
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        mut adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        match msg {
            ToStaticNeighbourhoodsAsMelody::SelectScale { index, time } => {
                if index != self.curr_scale_index {
                    self.curr_scale_index = index;
                    self.reset_scale_and_tunings(time, adaptor)
                } else {
                    adaptor
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReference { reference, time } => {
                let b;
                (b, adaptor) = self.set_reference(reference, adaptor);
                if b {
                    self.update_all_tunings_and_send(time, adaptor)
                } else {
                    adaptor
                }
            }
            ToStaticNeighbourhoodsAsMelody::UpdateScales {
                only_this_scale,
                time,
            } => match only_this_scale {
                None {} => {
                    (_, adaptor) = adaptor.config(|config, _| {
                        self.scales = config.scales.iter().map(|n| n.named.clone()).collect();
                    });
                    if self.scales.len() <= self.curr_scale_index {
                        self.curr_scale_index = 0;
                    }
                    self.reset_scale_and_tunings(time, adaptor)
                }
                Some(i) => {
                    (_, adaptor) = adaptor
                        .config(|config, _| self.scales[i].clone_from(&config.scales[i].named));
                    if i == self.curr_scale_index {
                        self.reset_scale_and_tunings(time, adaptor)
                    } else {
                        adaptor
                    }
                }
            },
            ToStaticNeighbourhoodsAsMelody::Reanchor { time } => {
                self.update_all_tunings_and_send(time, adaptor)
            }
        }
    }

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg> {
        match msg {
            ToMelody::StaticNeighbourhoods(msg) => Some(msg),
        }
    }

    // Make sure that [StrategyConfig::reacts_to_bound] exposes exactly the actions that this
    // function handles!
    fn handle_bound_action(
        &mut self,
        action: &BindableStrategyAction,
        time: Instant,
        mut adaptor: MelodyAdaptor<T, Self, Zero>,
    ) -> MelodyAdaptor<T, Self, Zero> {
        match action {
            BindableStrategyAction::IncrementNeighbourhoodIndex(increment) => {
                let old_index = self.curr_scale_index;
                self.curr_scale_index = (old_index as isize + increment)
                    .rem_euclid(self.scales.len() as isize)
                    as usize;
                if old_index != self.curr_scale_index {
                    self.reset_scale_and_tunings(time, adaptor)
                } else {
                    adaptor
                }
            }
            BindableStrategyAction::SetReferenceToLowest => {
                let b;
                (b, adaptor) = self.set_reference_to_extreme(false, adaptor);
                if b {
                    self.update_all_tunings_and_send(time, adaptor)
                } else {
                    adaptor
                }
            }
            BindableStrategyAction::SetReferenceToHighest => {
                let b;
                (b, adaptor) = self.set_reference_to_extreme(true, adaptor);
                if b {
                    self.update_all_tunings_and_send(time, adaptor)
                } else {
                    adaptor
                }
            }
            BindableStrategyAction::SetReferenceToCurrent => {
                let b;
                (b, adaptor) = self.set_reference_to_current(adaptor);
                if b {
                    self.update_all_tunings_and_send(time, adaptor)
                } else {
                    adaptor
                }
            }
            _ => adaptor,
        }
    }
}
