use std::{
    sync::mpsc,
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
    util::readerwriter::{Reader, ReaderWriter},
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
    pub fn new(
        config: NeighbourhoodsConfig<T>,
        forward: mpsc::Sender<FromStrategy<T>>,
        key_states: Reader<[KeyState; 128]>,
        tunings: ReaderWriter<[Stack<T>; 128]>,
    ) -> Self {
        Self {
            fixed: config.fixed,
            last_solve: Instant::now(),
            group_start_reference: config.inner.reference.clone(),
            group_duration: Duration::from_millis(config.group_ms),
            inner: StaticTuning::new(config.inner, forward, key_states, tunings),
        }
    }
}

impl<T: StackType> StaticTuning<T> {
    fn update_tunings_from_harmony(
        &mut self,
        harmony: Option<Harmony<T>>,
        time: Instant,
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
                if self.key_states.read()[i].is_sounding() {
                    let send_retune: bool;
                    if neighbourhood.try_write_relative_stack(
                        &mut self.tunings.write()[i],
                        i as StackCoeff - reference,
                    ) {
                        self.tunings.write()[i].scaled_add(1, &reference_tuning);
                        self.mark_tuning_as_outdated(i as u8);
                        send_retune = true;
                    } else {
                        send_retune = self.update_tuning(i as u8) == Some(true);
                    }
                    if send_retune {
                        let _ = self.forward.send(FromStrategy::Retune {
                            note: i as u8,
                            tuning: self.tunings.read()[i]
                                .absolute_semitones(self.tuning_reference.c4_semitones()),
                            tuning_stack: self.tunings.read()[i].clone(),
                            time,
                        });
                    }
                }
            }
            (true, Some(reference_tuning))
        } else {
            (self.update_all_tunings_and_send(time) >= 0, None {})
        }
    }
}

impl<T: StackType> MelodyStrategy<T> for Neighbourhoods<T> {
    fn solve(&mut self, harmony: Option<Harmony<T>>, time: Instant) -> (bool, Option<Stack<T>>) {
        let new_group = time.duration_since(self.last_solve) > self.group_duration;
        self.last_solve = time;

        if !self.fixed {
            if new_group {
                self.last_solve = time;
                self.group_start_reference.clone_from(&self.inner.reference);
            } else {
                self.inner.set_reference_to(&self.group_start_reference);
                self.inner.mark_all_tunings_as_outdated();
            }
        }

        let (success, new_reference) = self.inner.update_tunings_from_harmony(harmony, time);

        if !self.fixed {
            if let Some(new_reference) = &new_reference {
                self.inner.set_reference_to(new_reference);
            }
        }

        (success, new_reference)
    }

    fn handle_msg(
        &mut self,
        harmony: Option<Harmony<T>>,
        msg: ToStrategy<T>,
    ) -> (bool, Option<Stack<T>>) {
        match msg {
            ToStrategy::ReanchorOnMatch { reanchor } => {
                self.fixed = !reanchor;
                let _ = self
                    .inner
                    .forward
                    .send(FromStrategy::ReanchorOnMatch { reanchor });
                (true, Some(self.inner.reference.clone()))
            }
            ToStrategy::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms);
                (true, Some(self.inner.reference.clone()))
            }
            _ => {
                if let Some(time) = self.inner.handle_msg_but_dont_retune(msg) {
                    self.solve(harmony, time)
                } else {
                    (
                        true,
                        harmony.map(|h| self.inner.tunings.read()[h.reference as usize].clone()),
                    )
                }
            }
        }
    }

    fn start(&mut self, harmony: Option<Harmony<T>>, time: Instant) -> Option<Stack<T>> {
        self.inner.start_but_dont_retune();
        self.solve(harmony, time).1
    }

    fn absolute_semitones(&self, stack: &Stack<T>) -> Semitones {
        stack.absolute_semitones(self.inner.tuning_reference.c4_semitones())
    }

    fn handle_action(
        &mut self,
        harmony: Option<Harmony<T>>,
        action: StrategyAction,
        time: Instant,
    ) -> (bool, Option<Stack<T>>) {
        match action {
            StrategyAction::SetReferenceToCurrent => {
                if self.fixed {
                    self.fixed = false;
                    let res = self.solve(harmony, time);
                    self.fixed = true;
                    res
                } else {
                    self.solve(harmony, time)
                }
            }
            StrategyAction::ToggleReanchor => {
                self.fixed = !self.fixed;
                let _ = self.inner.forward.send(FromStrategy::ReanchorOnMatch {
                    reanchor: !self.fixed,
                });
                self.solve(harmony, time)
            }
            _ => {
                self.inner.handle_action(action, time);
                self.solve(harmony, time)
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
