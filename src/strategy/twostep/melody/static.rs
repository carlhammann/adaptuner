use std::{collections::VecDeque, time::Instant};

use crate::{
    config::{ExtractConfig, MelodyStrategyConfig, StrategyConfig},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::StackType,
    },
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
    neighbourhood::Neighbourhood,
    strategy::r#static::StaticTuning,
};

use super::super::{Harmony, MelodyStrategy};

impl<T: StackType> StaticTuning<T> {
    fn update_tunings_from_harmony(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        if let Some(Harmony {
            neighbourhood,
            reference,
        }) = harmony
        {
            if !self.force_update_tuning(tunings, reference) {
                return (false, None {});
            };
            let reference_tuning = tunings[reference as usize].clone();
            for i in 0..128 {
                if keys[i].is_sounding() {
                    let tuning = &mut tunings[i];
                    if neighbourhood.try_write_relative_stack(tuning, i as i8 - reference as i8) {
                        tuning.scaled_add(1, &reference_tuning);
                        self.mark_tuning_as_manually_set(i as u8);
                        forward.push_back(FromStrategy::Retune {
                            note: i as u8,
                            tuning: self.absolute_semitones(tuning),
                            tuning_stack: tuning.clone(),
                            time,
                        });
                    } else {
                        self.force_update_tuning(tunings, i as u8);
                    }
                }
            }
            (true, Some(reference_tuning))
        } else {
            (
                self.update_all_tunings_and_send(keys, tunings, time, forward) >= 0,
                None {},
            )
        }
    }
}

impl<T: StackType> MelodyStrategy<T> for StaticTuning<T> {
    /// Will tune the `harmony.reference` according to the currently selected neighbourhood. For
    /// all other notes, applies the tunings in the `harmony.neighbourhood` relative to the
    /// `harmony.reference`, falling back to the base tunings of the currently selected
    /// neighbourhood.
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        self.update_tunings_from_harmony(keys, tunings, harmony, time, forward)
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        if let Some(time) = self.handle_msg_but_dont_retune(keys, tunings, msg, forward) {
            self.update_tunings_from_harmony(keys, tunings, harmony, time, forward)
        } else {
            (true, harmony.map(|h| tunings[h.reference as usize].clone()))
        }
    }

    fn absolute_semitones(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.tuning_reference.c4_semitones())
    }

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<Stack<T>> {
        self.start_but_dont_retune(forward);
        self.update_tunings_from_harmony(keys, tunings, harmony, time, forward)
            .1
    }
}

impl<T: StackType> ExtractConfig<MelodyStrategyConfig<T>> for StaticTuning<T> {
    fn extract_config(&self) -> MelodyStrategyConfig<T> {
        match self.extract_config() {
            StrategyConfig::StaticTuning(c) => MelodyStrategyConfig::StaticTuning(c),
            _ => unreachable!(),
        }
    }
}
