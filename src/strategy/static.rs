use std::{sync::mpsc, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::StrategyConfig,
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::r#trait::Strategy,
};

use super::r#trait::StrategyAction;

pub struct StaticTuning<T: IntervalBasis> {
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: Option<usize>,
    tuning_reference: Reference<T>,
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
    fn update_and_send_tuning(
        &mut self,
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if let Some(cni) = self.curr_neighbourhood_index {
            if !self.tuning_up_to_date[note as usize] {
                self.neighbourhoods[cni].write_relative_stack(
                    tunings.get_mut(note as usize).unwrap(),
                    note as i8 - self.reference.key_number() as i8,
                );
                tunings
                    .get_mut(note as usize)
                    .unwrap()
                    .scaled_add(1, &self.reference);
                self.tuning_up_to_date[note as usize] = true;

                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Retune {
                    note,
                    tuning: tunings[note as usize]
                        .absolute_semitones(self.tuning_reference.c4_semitones()),
                    tuning_stack: tunings[note as usize].clone(),
                    time,
                }));
            }
        }
    }

    fn retune_all(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        for b in self.tuning_up_to_date.iter_mut() {
            *b = false;
        }
        for note in 0..128 {
            if keys[note as usize].is_sounding() {
                self.update_and_send_tuning(tunings, note, time, forward);
            }
        }
    }

    fn action(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        action: StrategyAction,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        match action {
            StrategyAction::IncrementNeighbourhoodIndex(inc) => {
                self.increment_neighbourhood(inc, keys, tunings, time, forward)
            }
            StrategyAction::SetReferenceToLowest => {
                self.set_reference(keys, tunings, time, forward)
            }
            StrategyAction::SetReferenceToHighest => todo!(),
        }
    }

    fn increment_neighbourhood(
        &mut self,
        increment: isize,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        if let Some(cni) = self.curr_neighbourhood_index {
            self.curr_neighbourhood_index = Some(
                (cni as isize + increment).rem_euclid(self.neighbourhoods.len() as isize) as usize,
            );
            self.start(keys, tunings, time, forward);
            true
        } else {
            false
        }
    }

    /// sets the reference to the lowest sounding note, or does nothing if no notes are currently
    /// sounding
    fn set_reference(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        for (i, state) in keys.iter().enumerate() {
            if state.is_sounding() {
                let new_reference = tunings[i].clone();
                self.reference.clone_from(&new_reference);
                self.retune_all(keys, tunings, time, forward);
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::SetReference {
                    stack: new_reference,
                }));
                return true;
            }
        }
        false
    }
}

impl<T: StackType> Strategy<T> for StaticTuning<T> {
    fn note_on<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)> {
        self.update_and_send_tuning(tunings, note, time, forward);
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
        _notes: &[u8],
        _time: Instant,
        _forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        true
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        match msg {
            ToStrategy::Consider {
                stack: considered_stack,
                time,
            } => {
                if let Some(cni) = self.curr_neighbourhood_index {
                    let inserted_stack = self.neighbourhoods[cni].insert(&considered_stack).clone();
                    self.retune_all(keys, tunings, time, forward); // todo can this be cheaper; retuning only what's needed?
                    let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                        stack: inserted_stack,
                    }));

                    true
                } else {
                    false
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
                        let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                            stack: stack.clone(),
                        }));
                    });
                    self.retune_all(keys, tunings, time, forward);
                } else {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.apply_temperament(temperament);
                    });
                }
                true
            }
            ToStrategy::MakeNeighbourhoodPure {
                time,
                neighbourhood,
            } => {
                if Some(neighbourhood) == self.curr_neighbourhood_index {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.make_pure();
                        let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                            stack: stack.clone(),
                        }));
                    });
                    self.retune_all(keys, tunings, time, forward);
                } else {
                    self.neighbourhoods[neighbourhood].for_each_stack_mut(|_, stack| {
                        stack.make_pure();
                    });
                }
                true
            }
            ToStrategy::SetTuningReference { reference, time } => {
                self.tuning_reference.clone_from(&reference);
                self.retune_all(keys, tunings, time, forward);
                let _ = forward.send(FromProcess::FromStrategy(
                    FromStrategy::SetTuningReference { reference },
                ));
                true
            }
            ToStrategy::SetReference { reference, time } => {
                self.reference.clone_from(&reference);
                self.retune_all(keys, tunings, time, forward);
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::SetReference {
                    stack: reference,
                }));
                true
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
                self.start(keys, tunings, time, forward);
                true
            }
        }
    }

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if let Some(cni) = self.curr_neighbourhood_index {
            let _ = forward.send(FromProcess::FromStrategy(
                FromStrategy::SetTuningReference {
                    reference: self.tuning_reference.clone(),
                },
            ));

            let _ = forward.send(FromProcess::FromStrategy(FromStrategy::SetReference {
                stack: self.reference.clone(),
            }));

            let _ = forward.send(FromProcess::FromStrategy(
                FromStrategy::CurrentNeighbourhoodIndex { index: cni },
            ));
            self.neighbourhoods[cni].for_each_stack(|_, stack| {
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                    stack: stack.clone(),
                }));
            });

            self.retune_all(keys, tunings, time, forward);
        }
    }

    fn extract_config(&self) -> StrategyConfig<T> {
        StrategyConfig::StaticTuning(StaticTuningConfig {
            neighbourhoods: self.neighbourhoods.clone(),
            tuning_reference: self.tuning_reference.clone(),
            reference: self.reference.clone(),
        })
    }
}
