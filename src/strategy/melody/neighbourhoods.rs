use std::time::{Duration, Instant};

use crate::{
    config::{ExtractConfig, IsMelodyStrategyConfig, MelodyStrategyConfig},
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToMelody, ToStaticNeighbourhoodsAsMelody},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::{
        harmony::r#trait::Harmony, melody::r#trait::MelodyStrategy, r#trait::StrategyAdaptor,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
    util::readerwriter::Reader,
};

#[derive(Clone)]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    pub reanchor: bool,
    pub inner: StaticNeighbourhoodsConfig<T>,
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
    type Realized = StaticNeighbourhoodsAsMelody<T>;

    fn as_melody_strategy_config(self) -> MelodyStrategyConfig<T> {
        MelodyStrategyConfig::Neighbourhoods(self)
    }
}

impl<T: StackType> ExtractConfig<StaticNeighbourhoodsAsMelodyConfig<T>>
    for StaticNeighbourhoodsAsMelody<T>
{
    fn extract_config(&self) -> StaticNeighbourhoodsAsMelodyConfig<T> {
        StaticNeighbourhoodsAsMelodyConfig {
            reanchor: self.reanchor,
            group_ms: self.group_duration.as_millis() as u64,
            inner: StaticNeighbourhoodsConfig {
                neighbourhoods: self.neighbourhoods.clone(),
                tuning_reference: self.tuning_reference.clone(),
                reference: self.reference.clone(),
            },
        }
    }
}

impl<T: StackType> StaticNeighbourhoodsAsMelody<T> {
    fn semitones_for_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.tuning_reference.c4_semitones())
    }

    fn tune_without_harmony(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) {
        adaptor.send(FromStrategy::CurrentHarmony {
            pattern_index: None {},
            reference: None {},
        });
        for i in 0..128 {
            if adaptor.read_key_state(i).is_sounding() {
                self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
                self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                    &mut adaptor.write_tuning(i).stack,
                    i as StackCoeff,
                    &self.reference,
                );

                let mut retune = self.tmp_stack != adaptor.read_tuning(i).stack;
                let the_tuning = self.semitones_for_stack(&adaptor.read_tuning(i).stack);
                if the_tuning != adaptor.read_tuning(i).tuning {
                    adaptor.write_tuning(i).tuning = the_tuning;
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
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
    ) {
        if self.reanchor {
            todo!()
        } else {
            let Harmony {
                neighbourhood: harmony_neighbourhood,
                reference: harmony_reference,
                pattern_index,
            } = &*harmony.read();
            adaptor.send(FromStrategy::CurrentHarmony {
                pattern_index: *pattern_index,
                reference: Some(
                    self.neighbourhoods[self.curr_neighbourhood_index]
                        .get_absolute_stack(*harmony_reference, &self.reference),
                ),
            });
            for i in 0..128 {
                if adaptor.read_key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
                    if harmony_neighbourhood.try_write_relative_stack(
                        &mut adaptor.write_tuning(i).stack,
                        i as StackCoeff - *harmony_reference,
                    ) {
                        self.neighbourhoods[self.curr_neighbourhood_index]
                            .increment_by_absolute_stack(
                                &mut adaptor.write_tuning(i).stack,
                                *harmony_reference,
                                &self.reference,
                            );
                    } else {
                        self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                            &mut adaptor.write_tuning(i).stack,
                            i as StackCoeff,
                            &self.reference,
                        );
                    }

                    let mut retune = self.tmp_stack != adaptor.read_tuning(i).stack;
                    let the_tuning = self.semitones_for_stack(&adaptor.read_tuning(i).stack);
                    if the_tuning != adaptor.read_tuning(i).tuning {
                        adaptor.write_tuning(i).tuning = the_tuning;
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
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    ) {
        if harmony_is_valid {
            self.tune_with_valid_harmony(time, adaptor, harmony);
        } else {
            self.tune_without_harmony(time, adaptor);
        }
    }

    /// returns true iff the reference changed
    fn set_reference(
        &mut self,
        adaptor: &impl StrategyAdaptor<T>,
        new_reference: Stack<T>,
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
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    ) -> bool {
        if harmony_is_valid {
            self.neighbourhoods[self.curr_neighbourhood_index].write_absolute_stack(
                &mut self.tmp_stack,
                harmony.read().reference,
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
        adaptor: &impl StrategyAdaptor<T>,
    ) -> bool {
        self.tmp_stack.clone_from(&self.reference);

        if to_highest {
            for i in (0..128).rev() {
                let state = &adaptor.read_key_state(i);
                if state.is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
                    break;
                }
            }
        } else {
            for i in 0..128 {
                let state = &adaptor.read_key_state(i);
                if state.is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
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

impl<T: StackType> MelodyStrategy<T> for StaticNeighbourhoodsAsMelody<T> {
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(config: Self::Config) -> Self {
        Self {
            neighbourhoods: config.inner.neighbourhoods,
            curr_neighbourhood_index: 0,
            tuning_reference: config.inner.tuning_reference,
            reference: config.inner.reference,
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
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    ) {
        self.udpate_all_tunings_and_send(time, adaptor, harmony, harmony_is_valid);
    }

    fn stop(&mut self, _time: Instant, _adaptor: &impl StrategyAdaptor<T>) {}

    fn start(
        &mut self,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    ) {
        adaptor.send(FromStrategy::SetTuningReference {
            reference: self.tuning_reference.clone(),
        });
        adaptor.send(FromStrategy::SetReference {
            stack: self.reference.clone(),
        });
        adaptor.send(FromStrategy::CurrentNeighbourhoodIndex {
            index: self.curr_neighbourhood_index,
        });
        self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
            adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });

        self.tune_with_harmony(time, adaptor, harmony, harmony_is_valid);
    }

    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
        _harmony: &impl Reader<Harmony<T>>,
        _harmony_is_valid: bool,
    ) {
        adaptor.send(FromStrategy::SetTuningReference {
            reference: reference.clone(),
        });

        self.tuning_reference = reference;
        for i in 0..128 {
            let state = adaptor.read_key_state(i);
            if state.is_sounding() {
                // do it like this to avoid double-locking 'adaptor.tunings'
                let mut x = adaptor.write_tuning(i);
                x.tuning = self.semitones_for_stack(&x.stack);
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        }
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: &impl StrategyAdaptor<T>,
        harmony: &impl Reader<Harmony<T>>,
        harmony_is_valid: bool,
    ) {
        match msg {
            ToStaticNeighbourhoodsAsMelody::SetReference { reference, time } => {
                if self.set_reference(adaptor, reference) {
                    self.udpate_all_tunings_and_send(time, adaptor, harmony, harmony_is_valid);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToLowest { time } => {
                if self.set_reference_to_extreme(false, adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor, harmony, harmony_is_valid);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToHighest { time } => {
                if self.set_reference_to_extreme(true, adaptor) {
                    self.udpate_all_tunings_and_send(time, adaptor, harmony, harmony_is_valid);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReferenceToCurrent { time } => {
                if self.set_reference_to_current(adaptor, harmony, harmony_is_valid) {
                    self.udpate_all_tunings_and_send(time, adaptor, harmony, harmony_is_valid);
                }
            }
            ToStaticNeighbourhoodsAsMelody::ToggleReanchor { time } => self.toggle_reanchor(time),
            ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms)
            }
            ToStaticNeighbourhoodsAsMelody::Consider { stack, time } => todo!(),
            ToStaticNeighbourhoodsAsMelody::ApplyTemperamentToNeighbourhood {
                neighbourhood,
                temperament,
                time,
            } => todo!(),
            ToStaticNeighbourhoodsAsMelody::MakeNeighbourhoodPure {
                neighbourhood,
                time,
            } => todo!(),
            ToStaticNeighbourhoodsAsMelody::NeighbourhoodListAction { action, time } => todo!(),
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
