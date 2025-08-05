use std::{collections::VecDeque, time::Instant};

use crate::{
    config::{ExtractConfig, MelodyStrategyConfig, StrategyConfig},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
    neighbourhood::Neighbourhood,
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::StrategyAction,
    },
};

use super::super::{Harmony, MelodyStrategy};

#[derive(Clone)]
pub struct NeighbourhoodsConfig<T: IntervalBasis> {
    pub fixed: bool,
    pub inner: StaticTuningConfig<T>,
}

pub struct Neighbourhoods<T: StackType> {
    fixed: bool,
    inner: StaticTuning<T>,
}

impl<T: StackType> Neighbourhoods<T> {
    pub fn new(config: NeighbourhoodsConfig<T>) -> Self {
        Self {
            fixed: config.fixed,
            inner: StaticTuning::new(config.inner),
        }
    }
}

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
            let Some(reference_tuning) = self.compute_tuning_for(reference) else {
                return (false, None {});
            };
            for i in 0..128 {
                if keys[i].is_sounding() {
                    let tuning = &mut tunings[i];
                    if neighbourhood.try_write_relative_stack(tuning, i as StackCoeff - reference) {
                        tuning.scaled_add(1, &reference_tuning);
                        self.mark_tuning_as_manually_set(i as u8);
                        forward.push_back(FromStrategy::Retune {
                            note: i as u8,
                            tuning: tuning.absolute_semitones(self.tuning_reference.c4_semitones()),
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

impl<T: StackType> MelodyStrategy<T> for Neighbourhoods<T> {
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
        let (success, new_reference) = self
            .inner
            .update_tunings_from_harmony(keys, tunings, harmony, time, forward);
        if !self.fixed {
            if let Some(new_reference) = &new_reference {
                self.inner.set_reference_to(new_reference, forward);
            }
        }
        (success, new_reference)
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        if let Some(time) = self
            .inner
            .handle_msg_but_dont_retune(keys, tunings, msg, forward)
        {
            self.solve(keys, tunings, harmony, time, forward)
        } else {
            (true, harmony.map(|h| tunings[h.reference as usize].clone()))
        }
    }

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<Stack<T>> {
        self.inner.start_but_dont_retune(forward);
        self.solve(keys, tunings, harmony, time, forward).1
    }

    fn absolute_semitones(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.inner.tuning_reference.c4_semitones())
    }

    fn handle_action(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
        action: StrategyAction,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) {
        self.inner
            .handle_action(keys, tunings, action, time, forward);
    }
}

impl<T: StackType> ExtractConfig<MelodyStrategyConfig<T>> for Neighbourhoods<T> {
    fn extract_config(&self) -> MelodyStrategyConfig<T> {
        match self.inner.extract_config() {
            StrategyConfig::StaticTuning(c) => {
                MelodyStrategyConfig::Neighbourhoods(NeighbourhoodsConfig {
                    fixed: self.fixed,
                    inner: c,
                })
            }
            _ => unreachable!(),
        }
    }
}
