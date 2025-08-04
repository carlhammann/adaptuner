use std::{collections::VecDeque, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, StrategyConfig},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::r#trait::Strategy,
};

use super::r#trait::StrategyAction;

pub struct StaticTuning<T: IntervalBasis> {
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: Option<usize>,
    pub tuning_reference: Reference<T>,
    reference: Stack<T>,
    tuning_up_to_date: [bool; 128],
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticTuningConfig<T: IntervalBasis> {
    pub neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,
}

impl<T: IntervalBasis> StaticTuning<T> {
    pub fn new(config: StaticTuningConfig<T>) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods,
            curr_neighbourhood_index: Some(0),
            tuning_reference: config.tuning_reference,
            reference: config.reference,
            tuning_up_to_date: [false; 128],
        }
    }
}

impl<T: StackType> StaticTuning<T> {
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

    /// Returns `true` iff the tuning was successfully updated (this will always be the case if
    /// there's a selected neighbourhood).
    pub fn force_update_tuning(&mut self, tunings: &mut [Stack<T>; 128], note: u8) -> bool {
        self.tuning_up_to_date[note as usize] = false;
        self.update_tuning(tunings, note).is_some()
    }

    /// Returns `Some` iff the tuning was successfully updated (this will always be the case if
    /// long as there's a selected neighbourhood),
    ///
    /// `Some(true)` means the tuning wasn't previously up to date.
    fn update_tuning(&mut self, tunings: &mut [Stack<T>; 128], note: u8) -> Option<bool> {
        if let Some(cni) = self.curr_neighbourhood_index {
            if !self.tuning_up_to_date[note as usize] {
                self.neighbourhoods[cni].write_relative_stack(
                    tunings.get_mut(note as usize).unwrap(),
                    note as StackCoeff - self.reference.key_number(),
                );
                tunings
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

    pub fn mark_tuning_as_manually_set(&mut self, note: u8) {
        self.tuning_up_to_date[note as usize] = false;
    }

    fn update_tuning_and_send(
        &mut self,
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        if let Some(changed) = self.update_tuning(tunings, note) {
            if changed {
                forward.push_back(FromStrategy::Retune {
                    note,
                    tuning: tunings[note as usize]
                        .absolute_semitones(self.tuning_reference.c4_semitones()),
                    tuning_stack: tunings[note as usize].clone(),
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
    pub fn update_all_tunings_and_send(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> isize {
        for b in self.tuning_up_to_date.iter_mut() {
            *b = false;
        }
        for note in 0..128 {
            if keys[note as usize].is_sounding() {
                if !self.update_tuning_and_send(tunings, note, time, forward) {
                    return note as isize - 1;
                }
            }
        }
        128
    }

    pub fn start_but_dont_retune(&mut self, forward: &mut VecDeque<FromStrategy<T>>) {
        if let Some(cni) = self.curr_neighbourhood_index {
            forward.push_back(FromStrategy::SetTuningReference {
                reference: self.tuning_reference.clone(),
            });

            forward.push_back(FromStrategy::SetReference {
                stack: self.reference.clone(),
            });

            forward.push_back(FromStrategy::CurrentNeighbourhoodIndex { index: cni });
            self.neighbourhoods[cni].for_each_stack(|_, stack| {
                forward.push_back(FromStrategy::Consider {
                    stack: stack.clone(),
                });
            });
        }
    }

    fn action(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
        action: StrategyAction,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<Instant> {
        if match action {
            StrategyAction::IncrementNeighbourhoodIndex(inc) => {
                self.increment_neighbourhood(inc, forward)
            }
            StrategyAction::SetReferenceToLowest => {
                self.set_reference(false, keys, tunings, forward)
            }
            StrategyAction::SetReferenceToHighest => {
                self.set_reference(true, keys, tunings, forward)
            }
        } {
            Some(time)
        } else {
            None {}
        }
    }

    fn increment_neighbourhood(
        &mut self,
        increment: isize,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        if let Some(cni) = self.curr_neighbourhood_index {
            self.curr_neighbourhood_index = Some(
                (cni as isize + increment).rem_euclid(self.neighbourhoods.len() as isize) as usize,
            );
            self.start_but_dont_retune(forward);
            true
        } else {
            false
        }
    }

    fn set_reference(
        &mut self,
        to_highest: bool,
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        let range: Box<dyn Iterator<Item = usize>> = if to_highest {
            Box::new((0..128).rev())
        } else {
            Box::new(0..128)
        };
        for i in range {
            if keys[i].is_sounding() {
                let new_reference = tunings[i].clone();
                self.reference.clone_from(&new_reference);
                forward.push_back(FromStrategy::SetReference {
                    stack: new_reference,
                });
                return true;
            }
        }
        false
    }

    /// returns `Some(x)` iff the message was successfully handled and a retune at time `x` is necessary
    pub fn handle_msg_but_dont_retune(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<Instant> {
        match msg {
            ToStrategy::Consider {
                stack: considered_stack,
                time,
            } => {
                if let Some(cni) = self.curr_neighbourhood_index {
                    let inserted_stack = self.neighbourhoods[cni].insert(&considered_stack).clone();
                    forward.push_back(FromStrategy::Consider {
                        stack: inserted_stack,
                    });

                    Some(time)
                } else {
                    None {}
                }
            }
            ToStrategy::ApplyTemperamentToNeighbourhood {
                temperament,
                neighbourhood,
                time,
            } => {
                if Some(neighbourhood) == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.apply_temperament(temperament);
                        forward.push_back(FromStrategy::Consider {
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
            ToStrategy::MakeNeighbourhoodPure {
                time,
                neighbourhood,
            } => {
                if Some(neighbourhood) == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.make_pure();
                        forward.push_back(FromStrategy::Consider {
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
            ToStrategy::SetTuningReference { reference, time } => {
                self.tuning_reference.clone_from(&reference);
                forward.push_back(FromStrategy::SetTuningReference { reference });
                Some(time)
            }
            ToStrategy::SetReference { reference, time } => {
                self.reference.clone_from(&reference);
                forward.push_back(FromStrategy::SetReference { stack: reference });
                Some(time)
            }
            ToStrategy::Action { action, time } => {
                self.action(keys, tunings, action, time, forward)
            }
            ToStrategy::NeighbourhoodListAction { action, time } => {
                action.apply_to(
                    |x| x.clone(),
                    &mut self.neighbourhoods,
                    &mut self.curr_neighbourhood_index,
                );
                self.start_but_dont_retune(forward);
                Some(time)
            }
            ToStrategy::ChordListAction { .. }
            | ToStrategy::PushNewChord { .. }
            | ToStrategy::AllowExtraHighNotes { .. }
            | ToStrategy::EnableChordList { .. } => unreachable!(),
        }
    }
}

impl<T: StackType> Strategy<T> for StaticTuning<T> {
    fn note_on<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)> {
        self.update_tuning_and_send(tunings, note, time, forward);
        let stack = &tunings[note as usize];
        Some((
            stack.absolute_semitones(self.tuning_reference.c4_semitones()),
            stack,
        ))
    }

    fn note_off(
        &mut self,
        _keys: &[KeyState; 128],
        _tunings: &mut [Stack<T>; 128],
        _note: u8,
        _time: Instant,
        _forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        true
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        if let Some(time) = self.handle_msg_but_dont_retune(keys, tunings, msg, forward) {
            self.update_all_tunings_and_send(keys, tunings, time, forward);
        }
        true
    }

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) {
        self.start_but_dont_retune(forward);
        self.update_all_tunings_and_send(keys, tunings, time, forward);
    }
}

impl<T: StackType> ExtractConfig<StrategyConfig<T>> for StaticTuning<T> {
    fn extract_config(&self) -> StrategyConfig<T> {
        StrategyConfig::StaticTuning(StaticTuningConfig {
            neighbourhoods: self.neighbourhoods.clone(),
            tuning_reference: self.tuning_reference.clone(),
            reference: self.reference.clone(),
        })
    }
}
