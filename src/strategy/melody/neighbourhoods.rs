use std::time::{Duration, Instant};

use crate::{
    config::{ExtractConfig, IsMelodyStrategyConfig, MelodyStrategyConfig},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
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

impl<T: StackType> StaticNeighbourhoodsAsMelody<T> {
    fn tuning_for_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.tuning_reference.c4_semitones())
    }

    /// calculate the tuning of note 'i' from the current neighbourhood and add it to the 'stack'
    fn add_reference_tuning(&self, stack: &mut Stack<T>, i: StackCoeff) {
        self.neighbourhoods[self.curr_neighbourhood_index]
            .increment_by_relative_stack(stack, i - self.reference.key_number());
        stack.scaled_add(1, &self.reference);
    }

    fn reference_tuning(&self, i: StackCoeff) -> Stack<T> {
        let mut res = Stack::new_zero();
        self.add_reference_tuning(&mut res, i);
        res
    }
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
        // if self.reanchor {
        //     todo!()
        // } else {
        if harmony_is_valid {
            let Harmony {
                neighbourhood: harmony_neighbourhood,
                reference: harmony_reference,
                pattern_index,
            } = &*harmony.read();
            adaptor.send(FromStrategy::CurrentHarmony {
                pattern_index: *pattern_index,
                reference: Some(self.reference_tuning(*harmony_reference as StackCoeff)),
            });
            for i in 0..128 {
                if adaptor.read_key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
                    if harmony_neighbourhood.try_write_relative_stack(
                        &mut adaptor.write_tuning(i).stack,
                        i as StackCoeff - *harmony_reference,
                    ) {
                        self.add_reference_tuning(
                            &mut adaptor.write_tuning(i).stack,
                            *harmony_reference as StackCoeff,
                        );
                    } else {
                        adaptor.write_tuning(i).stack.reset_to_zero();
                        self.add_reference_tuning(
                            &mut adaptor.write_tuning(i).stack,
                            i as StackCoeff,
                        );
                    }
                    if self.tmp_stack != adaptor.read_tuning(i).stack {
                        // It's important to lock the entry only once, and not use 'read_tuning'
                        // and 'write_tuning' in the same assigment. Otherwise, whichever lock is
                        // acquired first keeps the other open indefinitely...
                        let mut x = adaptor.write_tuning(i);
                        x.tuning = self.tuning_for_stack(&x.stack);
                        adaptor.send(FromStrategy::Retune {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
        } else {
            for i in 0..128 {
                if adaptor.read_key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.read_tuning(i).stack);
                    adaptor.write_tuning(i).stack.reset_to_zero();
                    self.add_reference_tuning(&mut adaptor.write_tuning(i).stack, i as StackCoeff);
                    if self.tmp_stack != adaptor.read_tuning(i).stack {
                        let mut x = adaptor.write_tuning(i);
                        x.tuning = self.tuning_for_stack(&x.stack);
                        adaptor.send(FromStrategy::Retune {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
        }
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
                let mut x = adaptor.write_tuning(i);
                x.tuning = self.tuning_for_stack(&x.stack);
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
        todo!()
    }

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg> {
        match msg {
            ToMelody::StaticNeighbourhoods(msg) => Some(msg),
        }
    }
}
