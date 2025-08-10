use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

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
    pub group_ms: u64,
}

pub struct Neighbourhoods<T: StackType> {
    fixed: bool,
    last_solve: Instant,
    group_start_reference: Stack<T>,
    group_duration: Duration,
    inner: StaticTuning<T>,
}

impl<T: StackType> Neighbourhoods<T> {
    pub fn new(config: NeighbourhoodsConfig<T>) -> Self {
        Self {
            fixed: config.fixed,
            last_solve: Instant::now(),
            group_start_reference: config.inner.reference.clone(),
            group_duration: Duration::from_millis(config.group_ms),
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
                    let send_retune: bool;
                    if neighbourhood
                        .try_write_relative_stack(&mut tunings[i], i as StackCoeff - reference)
                    {
                        tunings[i].scaled_add(1, &reference_tuning);
                        self.mark_tuning_as_outdated(i as u8);
                        send_retune = true;
                    } else {
                        send_retune = self.update_tuning(tunings, i as u8) == Some(true);
                    }
                    if send_retune {
                        forward.push_back(FromStrategy::Retune {
                            note: i as u8,
                            tuning: tunings[i]
                                .absolute_semitones(self.tuning_reference.c4_semitones()),
                            tuning_stack: tunings[i].clone(),
                            time,
                        });
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
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        let new_group = time.duration_since(self.last_solve) > self.group_duration;
        self.last_solve = time;

        if !self.fixed {
            if new_group {
                self.last_solve = time;
                self.group_start_reference.clone_from(&self.inner.reference);
            } else {
                self.inner
                    .set_reference_to(&self.group_start_reference, forward);
                self.inner.mark_all_tunings_as_outdated();
            }
        }

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
        match msg {
            ToStrategy::ReanchorOnMatch { reanchor } => {
                self.fixed = !reanchor;
                forward.push_back(FromStrategy::ReanchorOnMatch { reanchor });
                (true, Some(self.inner.reference.clone()))
            }
            ToStrategy::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms);
                (true, Some(self.inner.reference.clone()))
            }
            _ => {
                if let Some(time) = self
                    .inner
                    .handle_msg_but_dont_retune(keys, tunings, msg, forward)
                {
                    self.solve(keys, tunings, harmony, time, forward)
                } else {
                    (true, harmony.map(|h| tunings[h.reference as usize].clone()))
                }
            }
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
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        action: StrategyAction,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>) {
        match action {
            StrategyAction::SetReferenceToCurrent => {
                if self.fixed {
                    self.fixed = false;
                    let res = self.solve(keys, tunings, harmony, time, forward);
                    self.fixed = true;
                    res
                } else {
                    self.solve(keys, tunings, harmony, time, forward)
                }
            }
            StrategyAction::ToggleReanchor => {
                self.fixed = !self.fixed;
                forward.push_back(FromStrategy::ReanchorOnMatch {
                    reanchor: !self.fixed,
                });
                self.solve(keys, tunings, harmony, time, forward)
            }
            _ => {
                self.inner
                    .handle_action(keys, tunings, action, time, forward);
                self.solve(keys, tunings, harmony, time, forward)
            }
        }
    }
}

impl<T: StackType> ExtractConfig<MelodyStrategyConfig<T>> for Neighbourhoods<T> {
    fn extract_config(&self) -> MelodyStrategyConfig<T> {
        match self.inner.extract_config() {
            StrategyConfig::StaticTuning(c) => {
                MelodyStrategyConfig::Neighbourhoods(NeighbourhoodsConfig {
                    fixed: self.fixed,
                    inner: c,
                    group_ms: self.group_duration.as_millis() as u64,
                })
            }
            _ => unreachable!(),
        }
    }
}
