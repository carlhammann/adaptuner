use std::{
    ops::Deref,
    time::{Duration, Instant},
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{IsMelodyStrategyConfig, MelodyStrategyConfig, Named},
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToMelody, ToStaticNeighbourhoodsAsMelody},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::{
        harmony::r#trait::Harmony,
        melody::r#trait::{MelodyStrategy, MelodyStrategyAdaptor},
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    pub neighbourhoods: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,

    pub reanchor: bool,
    pub group_ms: u64,
}

/// The first three fields are exacly the same as for
/// [crate::strategy::staticneighbourhoods::StaticNeighbourhoods]
pub struct StaticNeighbourhoodsAsMelody<T: StackType> {
    /// This Vec must never be empty
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: usize,
    tuning_reference: Reference<T>,
    reference: Stack<T>,

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

    #[inline]
    fn tuning_reference(&self) -> &Reference<T> {
        &self.tuning_reference
    }
}

impl<T: StackType> StaticNeighbourhoodsAsMelody<T> {
    fn semitones_for_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.tuning_reference.c4_semitones())
    }

    fn tune_without_harmony(
        &mut self,
        time: Instant,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) {
        adaptor.send(FromStrategy::CurrentHarmony {
            pattern_index: None {},
            reference: None {},
        });
        for (i, state) in adaptor.key_states().iter().enumerate() {
            if state.is_sounding() {
                let the_tuning = &mut adaptor.tunings()[i];
                self.tmp_stack.clone_from(&the_tuning.stack);
                self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                    &mut the_tuning.stack,
                    i as StackCoeff,
                    &self.reference,
                );

                let mut retune = self.tmp_stack != the_tuning.stack;
                let new_semitones = self.semitones_for_stack(&the_tuning.stack);
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
            }
        }
    }

    fn tune_with_valid_harmony(
        &mut self,
        time: Instant,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) {
        if self.reanchor {
            todo!()
        } else {
            let Harmony {
                neighbourhood: harmony_neighbourhood,
                reference: harmony_reference,
                pattern_index,
                ..
            } = &*adaptor.harmony();
            adaptor.send(FromStrategy::CurrentHarmony {
                pattern_index: *pattern_index,
                reference: Some(
                    self.neighbourhoods[self.curr_neighbourhood_index]
                        .get_absolute_stack(*harmony_reference, &self.reference),
                ),
            });
            for (i, key_state) in adaptor.key_states().iter().enumerate() {
                if key_state.is_sounding() {
                    let the_tuning = &mut adaptor.tunings()[i];
                    self.tmp_stack.clone_from(&the_tuning.stack);
                    if harmony_neighbourhood.try_write_relative_stack(
                        &mut the_tuning.stack,
                        i as StackCoeff - *harmony_reference,
                    ) {
                        self.neighbourhoods[self.curr_neighbourhood_index]
                            .increment_by_absolute_stack(
                                &mut the_tuning.stack,
                                *harmony_reference,
                                &self.reference,
                            );
                    } else {
                        self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                            &mut the_tuning.stack,
                            i as StackCoeff,
                            &self.reference,
                        );
                    }

                    let mut retune = self.tmp_stack != the_tuning.stack;
                    let new_semitones = self.semitones_for_stack(&the_tuning.stack);
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
                }
            }
        }
    }

    fn udpate_all_tunings_and_send(
        &mut self,
        time: Instant,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) {
        if adaptor.harmony().valid {
            self.tune_with_valid_harmony(time, adaptor);
        } else {
            self.tune_without_harmony(time, adaptor);
        }
    }

    /// returns true iff the reference changed
    fn set_reference(
        &mut self,
        new_reference: Stack<T>,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) -> bool {
        if new_reference != self.reference {
            self.reference.clone_from(&new_reference);
            adaptor.send(FromStrategy::SetReference {
                stack: new_reference,
            });
            true
        } else {
            false
        }
    }

    /// returns true iff the reference changed
    fn set_reference_to_current(
        &mut self,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) -> bool {
        if adaptor.harmony().valid {
            self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                &mut self.tmp_stack,
                adaptor.harmony().reference,
                &self.reference,
            );

            if self.reference != self.tmp_stack {
                self.reference.clone_from(&self.tmp_stack);
                adaptor.send(FromStrategy::SetReference {
                    stack: self.reference.clone(),
                });
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// returns true iff the reference changed
    fn set_reference_to_extreme(
        &mut self,
        to_highest: bool,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) -> bool {
        self.tmp_stack.clone_from(&self.reference);

        if to_highest {
            for (i, state) in adaptor.key_states().iter().enumerate().rev() {
                if state.is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tunings()[i].stack);
                    break;
                }
            }
        } else {
            for (i, state) in adaptor.key_states().iter().enumerate() {
                if state.is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tunings()[i].stack);
                    break;
                }
            }
        }

        if self.reference != self.tmp_stack {
            self.reference.clone_from(&self.tmp_stack);
            adaptor.send(FromStrategy::SetReference {
                stack: self.reference.clone(),
            });
            true
        } else {
            false
        }
    }

    fn toggle_reanchor(&mut self, time: Instant) {
        self.reanchor = !self.reanchor;
        todo!()
    }
}

pub trait StaticNeighbourhoodsAsMelodyAdaptor<T: StackType>: MelodyStrategyAdaptor<T> {
    /// This function is allowed to be not extremely fast; it's only called in situations where we
    /// want to reload (parts of) the configuration.
    fn config(&self) -> impl Deref<Target = StaticNeighbourhoodsAsMelodyConfig<T>>;
}

impl<T: StackType, A: StaticNeighbourhoodsAsMelodyAdaptor<T>> MelodyStrategy<T, A>
    for StaticNeighbourhoodsAsMelody<T>
{
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(mut config: Self::Config) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods.drain(..).map(|n| n.named).collect(),
            curr_neighbourhood_index: 0,
            tuning_reference: config.tuning_reference,
            reference: config.reference,
            reanchor: config.reanchor,
            last_solve: Instant::now(),
            group_start_reference: Stack::new_zero(),
            group_duration: Duration::from_millis(config.group_ms),
            tmp_stack: Stack::new_zero(),
        }
    }

    fn tune_with_harmony(&mut self, time: Instant, adaptor: &A) {
        self.udpate_all_tunings_and_send(time, adaptor);
    }

    fn stop(&mut self, _time: Instant, _adaptor: &A) {}

    fn start(&mut self, time: Instant, adaptor: &A) {
        adaptor.send(FromStrategy::SetReference {
            stack: self.reference.clone(),
        });
        adaptor.send(FromStrategy::CurrentNeighbourhoodIndex {
            index: self.curr_neighbourhood_index,
        });
        self.tune_with_harmony(time, adaptor);
    }

    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A) {
        todo!() // copy what staticneighbourhoods does, this is incorrect:

        // for (i, state) in adaptor.key_states().iter().enumerate() {
        //     if state.is_sounding() {
        //         // do it like this to avoid double-locking 'adaptor.tunings'
        //         let x = &mut adaptor.tunings()[i];
        //         x.semitones = self.semitones_for_stack(&x.stack);
        //         adaptor.send(FromStrategy::Retune {
        //             note: i as u8,
        //             time,
        //         });
        //     }
        // }
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) {
        match msg {
            ToStaticNeighbourhoodsAsMelody::SetReference { reference, time } => {
                if self.set_reference(reference, adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToLowest { time } => {
                if self.set_reference_to_extreme(false, adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToHighest { time } => {
                if self.set_reference_to_extreme(true, adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToCurrent { time } => {
                if self.set_reference_to_current(adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::ToggleReanchor { time } => self.toggle_reanchor(time),
            ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms)
            }
            ToStaticNeighbourhoodsAsMelody::UpdateNeighbourhoods { time } => todo!(),
            ToStaticNeighbourhoodsAsMelody::IncrementNeighbourhoodIndex { increment, time } => {
                todo!()
            }
        }
    }

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg> {
        match msg {
            ToMelody::StaticNeighbourhoods(msg) => Some(msg),
        }
    }
}
