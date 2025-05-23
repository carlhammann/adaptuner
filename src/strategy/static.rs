use std::{sync::mpsc, time::Instant};

use crate::{
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{FiveLimitStackType, OctavePeriodicStackType, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, ToStrategy},
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
    tuning_up_to_date: [bool; 128],
}

#[derive(Clone)]
pub struct StaticTuningConfig<T: FiveLimitStackType + OctavePeriodicStackType> {
    pub active_temperaments: Vec<bool>,
    pub width: StackCoeff,
    pub index: StackCoeff,
    pub offset: StackCoeff,
    pub global_reference: Reference<T>,
}

impl<T: FiveLimitStackType + OctavePeriodicStackType> StaticTuning<T, PeriodicCompleteAligned<T>> {
    pub fn new(config: StaticTuningConfig<T>) -> Self {
        Self {
            neighbourhood: new_fivelimit_neighbourhood(
                &config.active_temperaments,
                config.width,
                config.index,
                config.offset,
            ),
            active_temperaments: config.active_temperaments.clone(),
            global_reference: config.global_reference.clone(),
            tuning_up_to_date: [false; 128],
        }
    }
}

impl<T: StackType, N: CompleteNeigbourhood<T> + PeriodicNeighbourhood<T>> StaticTuning<T, N> {
    fn update_and_send_tuning(
        &mut self,
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if !self.tuning_up_to_date[note as usize] {
            self.neighbourhood.write_relative_stack(
                tunings.get_mut(note as usize).unwrap(),
                note as i8 - self.global_reference.key as i8,
            );
            tunings
                .get_mut(note as usize)
                .unwrap()
                .scaled_add(1, &self.global_reference.stack);
            self.tuning_up_to_date[note as usize] = true;

            let _ = forward.send(FromProcess::FromStrategy(FromStrategy::Retune {
                note,
                tuning: tunings[note as usize]
                    .absolute_semitones(self.global_reference.c4_semitones()),
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

impl<T: StackType + std::fmt::Debug, N: CompleteNeigbourhood<T> + PeriodicNeighbourhood<T>>
    Strategy<T> for StaticTuning<T, N>
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
            stack.absolute_semitones(self.global_reference.c4_semitones()),
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
            ToStrategy::Consider { coefficients, time } => todo!(),
            ToStrategy::ToggleTemperament { index, time } => todo!(),
            ToStrategy::SetReference { reference, time } => {
                self.global_reference = reference;
                self.retune_all(keys, tunings, time, forward);
            }
        }
        true
    }
}
