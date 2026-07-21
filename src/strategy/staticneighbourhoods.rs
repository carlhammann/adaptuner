use std::{ops::Deref, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{IsStrategyConfig, Named},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToStaticNeighbourhoods, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    strategy::r#trait::{Strategy, StrategyAdaptor},
};

pub struct StaticNeighbourhoods<T: StackType> {
    /// this Vec must never be empty
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: usize,
    reference: Stack<T>,

    tmp_stack: Stack<T>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsConfig<T: IntervalBasis> {
    /// this Vec must never be empty
    pub neighbourhoods: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub initial_reference: Stack<T>,
}

impl<T: StackType> StaticNeighbourhoods<T> {
    /// Only does something iff the tuning stack, as accessed through the adaptor, changes. That
    /// is: You can't use this for retunes caused by a changing tuning reference.
    fn update_tuning_and_send(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: &impl StaticNeighbourhoodsAdaptor<T>,
    ) {
        let mut the_tuning = adaptor.tuning(note as usize);

        self.neighbourhoods[self.curr_neighbourhood_index].write_relative_stack(
            &mut self.tmp_stack,
            note as StackCoeff - self.reference.key_number(),
        );
        self.tmp_stack.scaled_add(1, &self.reference);

        let mut changed = false;

        if the_tuning.stack != self.tmp_stack {
            the_tuning.stack.clone_from(&self.tmp_stack);
            changed = true;
        }

        let the_semitones = self
            .tmp_stack
            .absolute_semitones(adaptor.tuning_reference().c4_semitones());
        if the_semitones != the_tuning.semitones {
            the_tuning.semitones = the_semitones;
            changed = true;
        }

        if changed {
            adaptor.send(FromStrategy::Retune { note, time });
        }
    }

    fn update_all_tunings_and_send(
        &mut self,
        time: Instant,
        adaptor: &impl StaticNeighbourhoodsAdaptor<T>,
    ) {
        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                self.update_tuning_and_send(i as u8, time, adaptor);
            }
        }
    }

    /// Returns true iff the reference changed. In that case, a re-tuning using
    /// [Self::update_all_tunings_and_send] will become necessary.
    fn set_reference_to_extreme(
        &mut self,
        to_highest: bool,
        adaptor: &impl StaticNeighbourhoodsAdaptor<T>,
    ) -> bool {
        self.tmp_stack.clone_from(&self.reference);

        if to_highest {
            for i in 0..128 {
                if adaptor.key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tuning(i).stack);
                    break;
                }
            }
        } else {
            for i in 0..128 {
                if adaptor.key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tuning(i).stack);
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
}

impl<T: StackType> IsStrategyConfig<T> for StaticNeighbourhoodsConfig<T> {}

pub trait StaticNeighbourhoodsAdaptor<T: StackType>: StrategyAdaptor<T> {
    /// This function is allowed to be not extremely fast; it's only called in situations where we
    /// want to reload (parts of) the configuration.
    fn config(&self) -> impl Deref<Target = StaticNeighbourhoodsConfig<T>>;
}

impl<T: StackType, A: StaticNeighbourhoodsAdaptor<T>> Strategy<T, A> for StaticNeighbourhoods<T> {
    type Msg = ToStaticNeighbourhoods<T>;
    type Config = StaticNeighbourhoodsConfig<T>;

    fn new(mut config: StaticNeighbourhoodsConfig<T>) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods.drain(..).map(|n| n.named).collect(),
            curr_neighbourhood_index: 0,
            reference: config.initial_reference,
            tmp_stack: Stack::new_zero(),
        }
    }

    fn start(&mut self, time: Instant, adaptor: &A) -> bool {
        adaptor.send(FromStrategy::SetReference {
            stack: self.reference.clone(),
        });

        adaptor.send(FromStrategy::CurrentNeighbourhoodIndex {
            index: self.curr_neighbourhood_index,
        });
        self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
            let _ = adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });

        self.update_all_tunings_and_send(time, adaptor);

        false
    }

    fn stop(&mut self, _time: Instant, _adaptor: &A) {}

    fn note_on(&mut self, note: u8, time: Instant, adaptor: &A) -> bool {
        self.update_tuning_and_send(note, time, adaptor);
        false
    }

    fn note_off(&mut self, _note: u8, _time: Instant, _adaptor: &A) -> bool {
        false
    }

    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A) -> bool {
        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                // do it like this to avoid double-locking 'adaptor.tunings'
                let x = &mut adaptor.tuning(i);
                x.semitones = x
                    .stack
                    .absolute_semitones(adaptor.tuning_reference().c4_semitones());
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        }
        false
    }

    fn receive_msg(&mut self, msg: ToStaticNeighbourhoods<T>, adaptor: &A) -> bool {
        match msg {
            ToStaticNeighbourhoods::UpdateNeighbourhoods { time } => {
                self.neighbourhoods = adaptor
                    .config()
                    .neighbourhoods
                    .iter()
                    .map(|n| n.named.clone())
                    .collect();
                if self.neighbourhoods.len() <= self.curr_neighbourhood_index {
                    self.curr_neighbourhood_index = 0;
                }
                self.start(time, adaptor);
            }
            ToStaticNeighbourhoods::SetReference { reference, time } => {
                self.reference.clone_from(&reference);
                let _ = adaptor.send(FromStrategy::SetReference { stack: reference });
                self.update_all_tunings_and_send(time, adaptor);
            }
            ToStaticNeighbourhoods::IncrementNeighbourhoodIndex { increment, time } => {
                let old_index = self.curr_neighbourhood_index;
                self.curr_neighbourhood_index = (old_index as isize + increment)
                    .rem_euclid(self.neighbourhoods.len() as isize)
                    as usize;
                if old_index != self.curr_neighbourhood_index {
                    self.start(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::SetReferenceToLowest { time } => {
                if self.set_reference_to_extreme(false, adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::SetReferenceToHighest { time } => {
                if self.set_reference_to_extreme(true, adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::Consider { stack, time } => {
                let inserted_stack = self.neighbourhoods[self.curr_neighbourhood_index]
                    .insert(&stack)
                    .clone();
                let _ = adaptor.send(FromStrategy::Consider {
                    stack: inserted_stack,
                });
                self.update_all_tunings_and_send(time, adaptor);
            }
        }
        false
    }

    fn step(&mut self, _adaptor: &A) -> bool {
        // no steps are needed for anything.
        false
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::StaticNeighbourhoods(msg) => Some(msg),
            _ => None {},
        }
    }
}
