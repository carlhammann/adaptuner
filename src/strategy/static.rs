use std::{sync::mpsc, time::Instant};

use crate::{
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::StackType,
    },
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, PeriodicNeighbourhood},
    reference::Reference,
    strategy::r#trait::Strategy,
};

pub struct StaticTuning<T: StackType, N: Neighbourhood<T>> {
    neighbourhoods: Vec<N>,
    curr_neighbourhood_index: usize,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
    tuning_up_to_date: [bool; 128],
}

impl<T: StackType, N: Neighbourhood<T>> StaticTuning<T, N> {
    pub fn new(
        tuning_reference: Reference<T>,
        initial_reference: Stack<T>,
        neighbourhoods: Vec<N>,
    ) -> Self {
        Self {
            neighbourhoods,
            curr_neighbourhood_index: 0,
            tuning_reference,
            reference: initial_reference,
            tuning_up_to_date: [false; 128],
        }
    }
}

impl<T: StackType, N: CompleteNeigbourhood<T>> StaticTuning<T, N> {
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
}

impl<
        T: StackType + std::fmt::Debug,
        N: CompleteNeigbourhood<T> + PeriodicNeighbourhood<T> + Clone,
    > Strategy<T> for StaticTuning<T, N>
{
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
                coefficients,
                temperaments,
                time,
            } => {
                let considered_stack = match temperaments {
                    None {} => &Stack::from_target(coefficients),
                    Some(v) => &Stack::from_temperaments_and_target(&v, coefficients),
                };
                let inserted_stack = self.neighbourhoods[self.curr_neighbourhood_index]
                    .insert(&considered_stack)
                    .clone();
                self.retune_all(keys, tunings, time, forward); // todo can this be cheaper; retuning only what's needed?
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Consider {
                    stack: inserted_stack,
                }));
            }
            ToStrategy::ToggleTemperament { index, time } => todo!(),
            ToStrategy::NextNeighbourhood { time } => {
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
                        name: self.neighbourhoods[self.curr_neighbourhood_index]
                            .name()
                            .into(),
                    },
                ));
            }
            ToStrategy::NewNeighbourhood { name } => {
                let mut new_neighbourhood = self.neighbourhoods[self.curr_neighbourhood_index].clone();
                new_neighbourhood.set_name(name);
                self.neighbourhoods.push(new_neighbourhood);
                self.curr_neighbourhood_index = self.neighbourhoods.len() - 1;
                let _ = forward.send(FromProcess::FromStrategy(
                    FromStrategy::CurrentNeighbourhoodName {
                        index: self.curr_neighbourhood_index,
                        name: self.neighbourhoods[self.curr_neighbourhood_index]
                            .name()
                            .into(),
                    },
                ));
            }
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
                        name: self.neighbourhoods[self.curr_neighbourhood_index]
                            .name()
                            .into(),
                    },
                ));
            }
            ToStrategy::SetTuningReference { reference, time } => {
                self.tuning_reference.clone_from(&reference);
                self.retune_all(keys, tunings, time, forward);
                let _ = forward.send(FromProcess::FromStrategy(
                    FromStrategy::SetTuningReference { reference },
                ));
            }
            ToStrategy::SetReference { reference, time } => {
                self.reference.clone_from(&reference);
                self.retune_all(keys, tunings, time, forward);
                let _ = forward.send(FromProcess::FromStrategy(FromStrategy::SetReference {
                    stack: reference,
                }));
            }
        }
        true
    }
}
