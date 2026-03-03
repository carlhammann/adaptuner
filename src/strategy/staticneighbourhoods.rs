use std::{collections::BTreeMap, ops::DerefMut, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    bindable::Bindable,
    config::{IsStrategyConfig, Named},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToStaticNeighbourhoods, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    process::r#trait::StackWithTuning,
    reference::Reference,
    strategy::r#trait::{Strategy, StrategyAction, StrategyAdaptor},
};

pub struct StaticNeighbourhoods<T: StackType> {
    /// this Vec must never be empty
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: usize,
    tuning_reference: Reference<T>,
    reference: Stack<T>,

    tmp_stack: Stack<T>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsConfig<T: IntervalBasis> {
    /// this Vec must never be empty
    pub neighbourhoods: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,

    pub bindings: BTreeMap<Bindable, StrategyAction>,
    pub name: String,
    pub description: String,
}

impl<T: StackType> StaticNeighbourhoods<T> {
    fn tuning_for_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.tuning_reference.c4_semitones())
    }

    /// Returns true iff the tuning stack or tuning semitones, as accessed through the adaptor,
    /// changed.
    fn update_tuning(
        &mut self,
        note: u8,
        mut tuning: impl DerefMut<Target = StackWithTuning<T>>,
    ) -> bool {
        self.neighbourhoods[self.curr_neighbourhood_index].write_relative_stack(
            &mut self.tmp_stack,
            note as StackCoeff - self.reference.key_number(),
        );
        self.tmp_stack.scaled_add(1, &self.reference);

        let mut changed = false;

        if tuning.stack != self.tmp_stack {
            tuning.stack.clone_from(&self.tmp_stack);
            changed = true;
        }

        let the_tuning = self.tuning_for_stack(&self.tmp_stack);
        if the_tuning != tuning.tuning {
            tuning.tuning = the_tuning;
            changed = true;
        }

        changed
    }

    /// Only does something iff the tuning stack, as accessed through the adaptor, changes. That
    /// is: You can't use this for retunes caused by a changing tuning reference.
    fn update_tuning_and_send(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
    ) {
        if self.update_tuning(note, adaptor.write_tuning(note as usize)) {
            adaptor.send(FromStrategy::Retune { note, time });
        }
    }

    fn update_all_tunings_and_send(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) {
        for i in 0..128 {
            let state = &adaptor.read_key_state(i);
            if state.is_sounding() {
                self.update_tuning_and_send(i as u8, time, adaptor);
            }
        }
    }

    /// Returns true iff the reference changed. In that case, a re-tuning using
    /// [Self::update_all_tunings_and_send] will become necessary.
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
}

impl<T: StackType> IsStrategyConfig<T> for StaticNeighbourhoodsConfig<T> {
    type Realized = StaticNeighbourhoods<T>;
}

impl<T: StackType> Strategy<T> for StaticNeighbourhoods<T> {
    type Msg = ToStaticNeighbourhoods<T>;
    type Config = StaticNeighbourhoodsConfig<T>;

    fn new(mut config: StaticNeighbourhoodsConfig<T>) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods.drain(..).map(|n| n.named).collect(),
            curr_neighbourhood_index: 0,
            tuning_reference: config.tuning_reference,
            reference: config.reference,
            tmp_stack: Stack::new_zero(),
        }
    }

    fn start(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
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
        self.update_all_tunings_and_send(time, adaptor);

        false
    }

    fn stop(&mut self, _time: Instant, _adaptor: &impl StrategyAdaptor<T>) {}

    fn reset(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        todo!()
        // self.start(time, adaptor)
    }

    fn note_on(&mut self, note: u8, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        if self.update_tuning(note, adaptor.write_tuning(note as usize)) {
            adaptor.send(FromStrategy::Retune { note, time });
        }
        false
    }

    fn note_off(&mut self, _note: u8, _time: Instant, _adaptor: &impl StrategyAdaptor<T>) -> bool {
        false
    }

    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
    ) -> bool {
        adaptor.send(FromStrategy::SetTuningReference {
            reference: reference.clone(),
        });

        self.tuning_reference = reference;
        for i in 0..128 {
            let state = adaptor.read_key_state(i);
            if state.is_sounding() {
                // do it like this to avoid double-locking 'adaptor.tunings'
                let mut x = adaptor.write_tuning(i);
                x.tuning = self.tuning_for_stack(&x.stack);
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        }
        false
    }

    fn receive_msg(
        &mut self,
        msg: ToStaticNeighbourhoods<T>,
        adaptor: &impl StrategyAdaptor<T>,
    ) -> bool {
        match msg {
            ToStaticNeighbourhoods::Consider { stack, time } => {
                let inserted_stack = self.neighbourhoods[self.curr_neighbourhood_index]
                    .insert(&stack)
                    .clone();
                let _ = adaptor.send(FromStrategy::Consider {
                    stack: inserted_stack,
                });
                self.update_all_tunings_and_send(time, adaptor);
            }
            ToStaticNeighbourhoods::ApplyTemperamentToNeighbourhood {
                neighbourhood,
                temperament,
                time,
            } => {
                self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                    stack.apply_temperament(temperament);
                });
                if neighbourhood == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        let _ = adaptor.send(FromStrategy::Consider {
                            stack: stack.clone(),
                        });
                    });
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::MakeNeighbourhoodPure {
                neighbourhood,
                time,
            } => {
                self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                    stack.make_pure();
                });
                if neighbourhood == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        let _ = adaptor.send(FromStrategy::Consider {
                            stack: stack.clone(),
                        });
                    });
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::NeighbourhoodListAction { action, time } => {
                let mut dummy = Some(self.curr_neighbourhood_index);
                action.apply_to(|x| x.clone(), &mut self.neighbourhoods, &mut dummy);
                self.curr_neighbourhood_index = dummy
                    .unwrap_or_else(|| panic!("there must alwazs remain a selected neighbourhood"));
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
        }
        false
    }

    fn step(&mut self, _adaptor: &impl StrategyAdaptor<T>) -> bool {
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
