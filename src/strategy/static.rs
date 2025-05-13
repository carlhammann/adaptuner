use std::time::Instant;

use crate::{
    config::r#trait::Config,
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{FiveLimitStackType, OctavePeriodicStackType, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg,
    neighbourhood::{
        new_fivelimit_neighbourhood, CompleteNeigbourhood, Neighbourhood, PeriodicCompleteAligned,
        PeriodicNeighbourhood,
    },
    reference::Reference,
    strategy::r#trait::Strategy,
};

pub struct StaticTuning<T: StackType, N: Neighbourhood<T>> {
    neighbourhood: N,
    active_temperaments: Vec<bool>,
    global_reference: Reference<T>,
}

impl<T: StackType, N: CompleteNeigbourhood<T> + PeriodicNeighbourhood<T>> Strategy<T>
    for StaticTuning<T, N>
{
    fn note_on<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        _time: Instant,
    ) -> Option<Vec<msg::FromStrategy<T>>> {
        self.neighbourhood.write_relative_stack(
            tunings
                .get_mut(note as usize)
                .expect("static strategy: note not in range 0..=127"),
            note as i8 - self.global_reference.key as i8,
        );

        //println!("relative: {}", tunings[note as usize].target);

        tunings
            .get_mut(note as usize)
            .unwrap()
            .scaled_add(1, &self.global_reference.stack);

        Some(vec![msg::FromStrategy::Retune {
            note,
            tuning: tunings[note as usize].absolute_semitones(),
            tuning_stack: tunings[note as usize].clone(),
        }])
    }

    fn note_off<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        _tunings: &'a mut [Stack<T>; 128],
        _note: &[u8],
        _time: Instant,
    ) -> Option<Vec<msg::FromStrategy<T>>> {
        Some(vec![])
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: msg::ToStrategy,
        _time: Instant,
    ) -> Option<Vec<msg::FromStrategy<T>>> {
        match msg {
            msg::ToStrategy::Consider { coefficients } => {
                let mut reference_stack =
                    Stack::from_temperaments_and_target(&self.active_temperaments, coefficients);
                let normalised_stack = self.neighbourhood.insert(&reference_stack);
                reference_stack.clone_from(normalised_stack);
                let mut res = vec![];

                let n = self.neighbourhood.period_keys() as StackCoeff;
                let r = reference_stack.key_number();

                for (note, state) in keys.iter().enumerate() {
                    if state.is_sounding() {
                        if (note as StackCoeff - r) % n == 0 {
                            tunings[note].clone_from(&reference_stack);
                            tunings[note].scaled_add(
                                (note as StackCoeff - r).div_euclid(n),
                                self.neighbourhood.period(),
                            );
                            res.push(msg::FromStrategy::Retune {
                                note: note as u8,
                                tuning: tunings[note].absolute_semitones(),
                                tuning_stack: tunings[note].clone(),
                            });
                        }
                    }
                }

                res.push(msg::FromStrategy::Consider {
                    stack: reference_stack,
                });

                Some(res)
            }
            msg::ToStrategy::ToggleTemperament { index } => {
                self.active_temperaments[index] = !self.active_temperaments[index];

                let mut res = vec![];

                self.neighbourhood.for_each_stack_mut(|_, stack| {
                    stack.retemper(&self.active_temperaments);
                    res.push(msg::FromStrategy::Consider {
                        stack: stack.clone(),
                    });
                });

                for (note, state) in keys.iter().enumerate() {
                    if state.is_sounding() {
                        self.neighbourhood.write_relative_stack(
                            tunings
                                .get_mut(note as usize)
                                .expect("static strategy: note not in range 0..=127"),
                            note as i8 - self.global_reference.key as i8,
                        );
                        tunings
                            .get_mut(note as usize)
                            .unwrap()
                            .scaled_add(1, &self.global_reference.stack);
                        res.push(msg::FromStrategy::Retune {
                            note: note as u8,
                            tuning: tunings[note].absolute_semitones(),
                            tuning_stack: tunings[note].clone(),
                        });
                    }
                }

                Some(res)
            }
        }
    }
}

#[derive(Clone)]
pub struct StaticTuningConfig<T: FiveLimitStackType + OctavePeriodicStackType> {
    pub active_temperaments: Vec<bool>,
    pub width: StackCoeff,
    pub index: StackCoeff,
    pub offset: StackCoeff,
    pub global_reference: Reference<T>
}

impl<T: FiveLimitStackType + OctavePeriodicStackType>
    Config<StaticTuning<T, PeriodicCompleteAligned<T>>> for StaticTuningConfig<T>
{
    fn initialise(config: &Self) -> StaticTuning<T, PeriodicCompleteAligned<T>> {
        StaticTuning {
            neighbourhood: new_fivelimit_neighbourhood(
                &config.active_temperaments,
                config.width,
                config.index,
                config.offset,
            ),
            active_temperaments: config.active_temperaments.clone(),
            global_reference: config.global_reference.clone()
        }
    }
}
