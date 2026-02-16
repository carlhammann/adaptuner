use std::{sync::mpsc, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, IsStrategyConfig, StrategyConfig},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, ReceiveMsg, ToStaticNeighbourhoods, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::r#trait::Strategy,
    util::readerwriter::{Reader, ReaderWriter},
};

use super::r#trait::StrategyAction;

pub struct StaticNeighbourhoods<T: StackType> {
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: Option<usize>,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
    tuning_up_to_date: [bool; 128],
    forward: mpsc::Sender<FromStrategy<T>>,
    key_states: Reader<[KeyState; 128]>,
    tunings: ReaderWriter<[Stack<T>; 128]>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsConfig<T: IntervalBasis> {
    pub neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,
}

impl<T: StackType> StaticNeighbourhoods<T> {
    /// Compute the tuning for a note (that may lie outside of the MIDI range). Returns `None` only
    /// in the case when there's no neighbourhood currently selected).
    pub fn compute_tuning_for(&self, note: StackCoeff) -> Option<Stack<T>> {
        if let Some(cni) = self.curr_neighbourhood_index {
            let mut res =
                self.neighbourhoods[cni].get_relative_stack(note - self.reference.key_number());
            res.scaled_add(1, &self.reference);
            Some(res)
        } else {
            None {}
        }
    }

    /// Returns `Some` iff the tuning was successfully updated (this will always be the case if
    /// long as there's a selected neighbourhood),
    ///
    /// `Some(true)` means the tuning wasn't previously up to date.
    pub fn update_tuning(&mut self, note: u8) -> Option<bool> {
        if let Some(cni) = self.curr_neighbourhood_index {
            if !self.tuning_up_to_date[note as usize] {
                self.neighbourhoods[cni].write_relative_stack(
                    self.tunings.write().get_mut(note as usize).unwrap(),
                    note as StackCoeff - self.reference.key_number(),
                );
                self.tunings
                    .write()
                    .get_mut(note as usize)
                    .unwrap()
                    .scaled_add(1, &self.reference);
                self.tuning_up_to_date[note as usize] = true;
                Some(true)
            } else {
                Some(false)
            }
        } else {
            None {}
        }
    }

    pub fn mark_tuning_as_outdated(&mut self, note: u8) {
        self.tuning_up_to_date[note as usize] = false;
    }

    pub fn mark_all_tunings_as_outdated(&mut self) {
        self.tuning_up_to_date.iter_mut().for_each(|b| *b = false);
    }

    fn update_tuning_and_send(&mut self, note: u8, time: Instant) -> bool {
        if let Some(changed) = self.update_tuning(note) {
            if changed {
                let _ = self.forward.send(FromStrategy::Retune {
                    note,
                    tuning: self.tunings.read()[note as usize]
                        .absolute_semitones(self.tuning_reference.c4_semitones()),
                    tuning_stack: self.tunings.read()[note as usize].clone(),
                    time,
                });
            }
            true
        } else {
            false
        }
    }

    /// returns the index of the highest note that was either successfully tuned or silent: 128
    /// means full successs, -1 means no note was tuned.
    pub fn update_all_tunings_and_send(&mut self, time: Instant) -> isize {
        for b in self.tuning_up_to_date.iter_mut() {
            *b = false;
        }
        for note in 0..128 {
            if self.key_states.read()[note as usize].is_sounding() {
                if !self.update_tuning_and_send(note, time) {
                    return note as isize - 1;
                }
            }
        }
        128
    }

    pub fn start_but_dont_retune(&mut self) {
        if let Some(cni) = self.curr_neighbourhood_index {
            let _ = self.forward.send(FromStrategy::SetTuningReference {
                reference: self.tuning_reference.clone(),
            });

            let _ = self.forward.send(FromStrategy::SetReference {
                stack: self.reference.clone(),
            });

            let _ = self
                .forward
                .send(FromStrategy::CurrentNeighbourhoodIndex { index: cni });
            self.neighbourhoods[cni].for_each_stack(|_, stack| {
                let _ = self.forward.send(FromStrategy::Consider {
                    stack: stack.clone(),
                });
            });
        }
    }

    /// returns true iff a retune becomes necessary
    fn increment_neighbourhood(&mut self, increment: isize) -> bool {
        if let Some(cni) = self.curr_neighbourhood_index {
            let old_index = cni;
            let new_index =
                (cni as isize + increment).rem_euclid(self.neighbourhoods.len() as isize) as usize;
            if old_index != new_index {
                self.curr_neighbourhood_index = Some(new_index);
                self.tuning_up_to_date.iter_mut().for_each(|b| *b = false);
                self.start_but_dont_retune();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// returns true iff a retune becomes necessary
    fn set_reference(&mut self, to_highest: bool) -> bool {
        let range: Box<dyn Iterator<Item = usize>> = if to_highest {
            Box::new((0..128).rev())
        } else {
            Box::new(0..128)
        };
        for i in range {
            if self.key_states.read()[i].is_sounding() {
                self.set_reference_to_index(i);
                return true;
            }
        }
        false
    }

    fn set_reference_to_index(&mut self, new_reference_index: usize) {
        self.reference
            .clone_from(&self.tunings.read()[new_reference_index]);
        let _ = self.forward.send(FromStrategy::SetReference {
            stack: self.tunings.read()[new_reference_index].clone(),
        });
    }

    pub fn set_reference_to(&mut self, new_reference: &Stack<T>) {
        self.reference.clone_from(new_reference);
        let _ = self.forward.send(FromStrategy::SetReference {
            stack: new_reference.clone(),
        });
    }

    /// returns `Some(x)` iff the message was successfully handled and a retune at time `x` is necessary
    fn receive_to_static_neighbourhoods_but_dont_retune(
        &mut self,
        msg: ToStaticNeighbourhoods<T>,
    ) -> Option<Instant> {
        match msg {
            ToStaticNeighbourhoods::Consider {
                stack: considered_stack,
                time,
            } => {
                if let Some(cni) = self.curr_neighbourhood_index {
                    let inserted_stack = self.neighbourhoods[cni].insert(&considered_stack).clone();
                    let _ = self.forward.send(FromStrategy::Consider {
                        stack: inserted_stack,
                    });

                    Some(time)
                } else {
                    None {}
                }
            }
            ToStaticNeighbourhoods::ApplyTemperamentToNeighbourhood {
                neighbourhood,
                temperament,
                time,
            } => {
                if Some(neighbourhood) == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.apply_temperament(temperament);
                        let _ = self.forward.send(FromStrategy::Consider {
                            stack: stack.clone(),
                        });
                    });
                    Some(time)
                } else {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.apply_temperament(temperament);
                    });
                    None {}
                }
            }
            ToStaticNeighbourhoods::MakeNeighbourhoodPure {
                neighbourhood,
                time,
            } => {
                if Some(neighbourhood) == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.make_pure();
                        let _ = self.forward.send(FromStrategy::Consider {
                            stack: stack.clone(),
                        });
                    });
                    Some(time)
                } else {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.make_pure();
                    });
                    None {}
                }
            }
            ToStaticNeighbourhoods::NeighbourhoodListAction { action, time } => {
                action.apply_to(
                    |x| x.clone(),
                    &mut self.neighbourhoods,
                    &mut self.curr_neighbourhood_index,
                );
                self.start_but_dont_retune();
                Some(time)
            }
            ToStaticNeighbourhoods::SetReference { reference, time } => {
                self.reference.clone_from(&reference);
                let _ = self
                    .forward
                    .send(FromStrategy::SetReference { stack: reference });
                Some(time)
            }
            ToStaticNeighbourhoods::IncrementNeighbourhoodIndex { increment, time } => {
                if self.increment_neighbourhood(increment) {
                    Some(time)
                } else {
                    None {}
                }
            }

            ToStaticNeighbourhoods::SetReferenceToLowest { time } => {
                if self.set_reference(false) {
                    Some(time)
                } else {
                    None {}
                }
            }
            ToStaticNeighbourhoods::SetReferenceToHighest { time } => {
                if self.set_reference(true) {
                    Some(time)
                } else {
                    None {}
                }
            }
        }
    }

    pub fn handle_action(&mut self, action: StrategyAction, time: Instant) -> Option<Instant> {
        if match action {
            StrategyAction::IncrementNeighbourhoodIndex(inc) => self.increment_neighbourhood(inc),
            StrategyAction::SetReferenceToLowest => self.set_reference(false),
            StrategyAction::SetReferenceToHighest => self.set_reference(true),
            StrategyAction::Reset => {
                self.curr_neighbourhood_index = if self.neighbourhoods.is_empty() {
                    None {}
                } else {
                    Some(0)
                };
                self.reference = Stack::new_zero();
                self.start_but_dont_retune();
                true
            }
            _ => false,
        } {
            Some(time)
        } else {
            None {}
        }
    }
}

impl<T: StackType> ReceiveMsg<ToStaticNeighbourhoods<T>> for StaticNeighbourhoods<T> {
    fn receive_msg(&mut self, msg: ToStaticNeighbourhoods<T>) {
        if let Some(retune_time) = self.receive_to_static_neighbourhoods_but_dont_retune(msg) {
            self.update_all_tunings_and_send(retune_time);
        }
    }
}

impl<T: StackType> IsStrategyConfig<T> for StaticNeighbourhoodsConfig<T> {
    type Realized = StaticNeighbourhoods<T>;

    fn as_strategy_config(self) -> StrategyConfig<T> {
        StrategyConfig::StaticTuning(self)
    }
}

impl<T: StackType> ExtractConfig<StaticNeighbourhoodsConfig<T>> for StaticNeighbourhoods<T> {
    fn extract_config(&self) -> StaticNeighbourhoodsConfig<T> {
        StaticNeighbourhoodsConfig {
            neighbourhoods: self.neighbourhoods.clone(),
            tuning_reference: self.tuning_reference.clone(),
            reference: self.reference.clone(),
        }
    }
}

impl<T: StackType> Strategy<T> for StaticNeighbourhoods<T> {
    type Msg = ToStaticNeighbourhoods<T>;
    type Config = StaticNeighbourhoodsConfig<T>;

    fn new(
        config: StaticNeighbourhoodsConfig<T>,
        forward: mpsc::Sender<FromStrategy<T>>,
        key_states: Reader<[KeyState; 128]>,
        tunings: ReaderWriter<[Stack<T>; 128]>,
    ) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods,
            curr_neighbourhood_index: Some(0),
            tuning_reference: config.tuning_reference,
            reference: config.reference,
            tuning_up_to_date: [false; 128],
            forward,
            key_states,
            tunings,
        }
    }

    fn note_on(&mut self, note: u8, time: Instant) {
        self.update_tuning_and_send(note, time);
        let stack = &self.tunings.read()[note as usize];
    }

    fn note_off(&mut self, _note: u8, _time: Instant) {}

    fn start(&mut self, time: Instant) {
        self.start_but_dont_retune();
        self.update_all_tunings_and_send(time);
    }

    fn stop(&mut self, time: Instant) {}

    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant) {
        self.tuning_reference.clone_from(&reference);
        let _ = self
            .forward
            .send(FromStrategy::SetTuningReference { reference });
        self.update_all_tunings_and_send(time);
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::StaticNeighbourhoods(msg) => Some(msg),
            _ => None {},
        }
    }
}
