use std::time::{Duration, Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, HarmonyLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel,
    },
    bindable::BindableStrategyAction,
    config::{IsMelodyStrategyConfig, MelodyStrategyConfig, Named, StrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToMelody, ToStaticNeighbourhoodsAsMelody},
    neighbourhood::{CompleteNeighbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    strategy::{
        harmony::r#trait::Harmony,
        melody::r#trait::{MelodyAdaptor, MelodyStrategy},
    },
    util::ordered_locks::{AtMost, Succ, Zero},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    pub scales: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub initial_reference: Stack<T>,

    pub reanchor: bool,
    pub group_ms: u64,
}

/// The first three fields are exacly the same as for
/// [crate::strategy::staticneighbourhoods::StaticNeighbourhoods]
pub struct StaticNeighbourhoodsAsMelody<T: StackType> {
    /// This Vec must never be empty
    scales: Vec<SomeCompleteNeighbourhood<T>>,
    curr_scale_index: usize,

    reanchor: bool,

    last_solve: Instant,
    group_start_reference: Stack<T>,
    group_duration: Duration,

    tmp_stack: Stack<T>,
}

impl<T: StackType> IsMelodyStrategyConfig<T> for StaticNeighbourhoodsAsMelodyConfig<T> {
    fn as_melody_strategy_config(self) -> MelodyStrategyConfig<T> {
        MelodyStrategyConfig::StaticNeighbourhoods(self)
    }
}

impl<T: StackType> StaticNeighbourhoodsAsMelody<T> {
    fn tune_without_harmony<L>(
        &mut self,
        time: Instant,
        adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<KeyStateLevel>,
    {
        adaptor.send(FromStrategy::UpdateHarmony {});
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

    fn tune_with_valid_harmony<L>(
        &mut self,
        time: Instant,
        harmony: &Harmony<T>,
        adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        if self.reanchor {
            todo!();
            adaptor
        } else {
            let Harmony {
                neighbourhood: harmony_neighbourhood,
                reference: harmony_reference,
                ..
            } = harmony;
            adaptor.send(FromStrategy::UpdateHarmony {});
            adaptor.for_all_sounding_tunings_mut(|i, the_tuning, mut adaptor| {
                self.tmp_stack.clone_from(&the_tuning.stack);
                if harmony_neighbourhood.try_write_relative_stack(
                    &mut the_tuning.stack,
                    i as StackCoeff - *harmony_reference,
                ) {
                    (_, adaptor) = adaptor.reference(|adaptor_reference, _| {
                        self.scales[self.curr_scale_index].increment_by_absolute_stack(
                            &mut the_tuning.stack,
                            *harmony_reference,
                            adaptor_reference,
                        )
                    });
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
            })
        }
    }

    fn update_all_tunings_and_send<L>(
        &mut self,
        time: Instant,
        mut adaptor: MelodyAdaptor<T, Self, L>,
    ) -> MelodyAdaptor<T, Self, L>
    where
        L: AtMost<HarmonyLevel>, // for the called sub-methods: + AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        (_, adaptor) = adaptor.harmony(|m_harmony, adaptor| match m_harmony {
            Some(harmony) => {
                if harmony.valid {
                    self.tune_with_valid_harmony(time, harmony, adaptor)
                } else {
                    self.tune_without_harmony(time, adaptor)
                }
            }
            None {} => self.tune_without_harmony(time, adaptor),
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
        adaptor.harmony(|m_harmony, adaptor| match m_harmony {
            Some(harmony) => {
                if harmony.valid {
                    adaptor
                        .reference_mut(|adaptor_reference, adaptor| {
                            self.scales[self.curr_scale_index].write_absolute_stack(
                                &mut self.tmp_stack,
                                harmony.reference,
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
            }
            None {} => false,
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

    fn toggle_reanchor(&mut self, _time: Instant) {
        todo!();
        self.reanchor = !self.reanchor;
    }
}

impl<T: StackType, L: AtMost<StrategyConfigLevel>>
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

impl<T: StackType> MelodyStrategy<T> for StaticNeighbourhoodsAsMelody<T> {
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(mut config: Self::Config) -> Self {
        Self {
            scales: config.scales.drain(..).map(|n| n.named).collect(),
            curr_scale_index: 0,
            reanchor: config.reanchor,
            last_solve: Instant::now(),
            group_start_reference: Stack::new_zero(),
            group_duration: Duration::from_millis(config.group_ms),
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
        adaptor.send(FromStrategy::UpdateReference {});
        adaptor.send(FromStrategy::SelectScale {
            index: self.curr_scale_index,
        });
        self.scales[self.curr_scale_index].for_each_stack(|_, stack| {
            let _ = adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });
        (_, adaptor) = adaptor.config(|config, adaptor| {
            adaptor.reference_mut(|reference, _| reference.clone_from(&config.initial_reference));
        });
        self.tune_with_harmony(time, adaptor)
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
                    self.start(time, adaptor)
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
            ToStaticNeighbourhoodsAsMelody::ToggleReanchor { time } => {
                self.toggle_reanchor(time);
                adaptor
            }
            ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms);
                adaptor
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
                    self.start(time, adaptor)
                }
                Some(i) => {
                    (_, adaptor) = adaptor
                        .config(|config, _| self.scales[i].clone_from(&config.scales[i].named));
                    if i == self.curr_scale_index {
                        self.start(time, adaptor)
                    } else {
                        adaptor
                    }
                }
            },
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
                    self.start(time, adaptor)
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
