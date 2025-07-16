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

pub struct StaticTuning<T: IntervalBasis> {
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: usize,
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
            curr_neighbourhood_index: 0,
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
        if !self.tuning_up_to_date[note as usize] {
            self.neighbourhoods[self.curr_neighbourhood_index].write_relative_stack(
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

    fn new_neighbourhood(&mut self, name: String, forward: &mpsc::Sender<FromProcess<T>>) -> bool {
        let mut new_neighbourhood = self.neighbourhoods[self.curr_neighbourhood_index].clone();
        new_neighbourhood.set_name(name);
        self.neighbourhoods.push(new_neighbourhood);
        self.curr_neighbourhood_index = self.neighbourhoods.len() - 1;
        let _ = forward.send(FromProcess::FromStrategy(
            FromStrategy::CurrentNeighbourhoodName {
                index: self.curr_neighbourhood_index,
                n_neighbourhoods: self.neighbourhoods.len(),
                name: self.neighbourhoods[self.curr_neighbourhood_index]
                    .name()
                    .into(),
            },
        ));
        self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
            let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                stack: stack.clone(),
            }));
        });

        true
    }
}

impl<T: StackType + std::fmt::Debug> Strategy<T> for StaticTuning<T> {
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
                let inserted_stack = self.neighbourhoods[self.curr_neighbourhood_index]
                    .insert(&considered_stack)
                    .clone();
                self.retune_all(keys, tunings, time, forward); // todo can this be cheaper; retuning only what's needed?
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                    stack: inserted_stack,
                }));

                true
            }
            ToStrategy::SetTemperaments { temperaments, time } => {
                self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack_mut(|_, stack| {
                    stack.retemper(&temperaments);
                    let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                        stack: stack.clone(),
                    }));
                });
                self.retune_all(keys, tunings, time, forward); 

                true
            }
            ToStrategy::NextNeighbourhood { time } => {
                self.next_neighbourhood(keys, tunings, time, forward)
            }
            ToStrategy::NewNeighbourhood { name } => self.new_neighbourhood(name, forward),
            ToStrategy::DeleteCurrentNeighbourhood { time } => {
                if self.neighbourhoods.len() < 2 {
                    return false;
                }

                self.neighbourhoods.remove(self.curr_neighbourhood_index);
                self.curr_neighbourhood_index =
                    self.curr_neighbourhood_index % self.neighbourhoods.len();

                self.retune_all(keys, tunings, time, forward);
                self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
                    let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                        stack: stack.clone(),
                    }));
                });
                let _ = forward.send(FromProcess::FromStrategy(
                    FromStrategy::CurrentNeighbourhoodName {
                        index: self.curr_neighbourhood_index,
                        n_neighbourhoods: self.neighbourhoods.len(),
                        name: self.neighbourhoods[self.curr_neighbourhood_index]
                            .name()
                            .into(),
                    },
                ));

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
        }
    }

    fn next_neighbourhood(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        self.curr_neighbourhood_index =
            (self.curr_neighbourhood_index + 1) % self.neighbourhoods.len();
        self.retune_all(keys, tunings, time, forward);
        self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
            let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                stack: stack.clone(),
            }));
        });
        let _ = forward.send(FromProcess::FromStrategy(
            FromStrategy::CurrentNeighbourhoodName {
                index: self.curr_neighbourhood_index,
                n_neighbourhoods: self.neighbourhoods.len(),
                name: self.neighbourhoods[self.curr_neighbourhood_index]
                    .name()
                    .into(),
            },
        ));

        true
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

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        let _ = forward.send(FromProcess::FromStrategy(
            FromStrategy::SetTuningReference {
                reference: self.tuning_reference.clone(),
            },
        ));

        let _ = forward.send(FromProcess::FromStrategy(FromStrategy::SetReference {
            stack: self.reference.clone(),
        }));

        let _ = forward.send(FromProcess::FromStrategy(
            FromStrategy::CurrentNeighbourhoodName {
                index: self.curr_neighbourhood_index,
                n_neighbourhoods: self.neighbourhoods.len(),
                name: self.neighbourhoods[self.curr_neighbourhood_index]
                    .name()
                    .into(),
            },
        ));
        self.neighbourhoods[self.curr_neighbourhood_index].for_each_stack(|_, stack| {
            let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                stack: stack.clone(),
            }));
        });

        self.retune_all(keys, tunings, time, forward);
    }

    fn extract_config(&self) -> StrategyConfig<T> {
        StrategyConfig::StaticTuning(StaticTuningConfig {
            neighbourhoods: self.neighbourhoods.clone(),
            tuning_reference: self.tuning_reference.clone(),
            reference: self.reference.clone(),
        })
    }
}
