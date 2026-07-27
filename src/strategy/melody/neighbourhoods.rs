use std::{
    ops::Deref,
    time::{Duration, Instant},
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    bindable::BindableStrategyAction,
    config::{IsMelodyStrategyConfig, MelodyStrategyConfig, Named},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToMelody, ToStaticNeighbourhoodsAsMelody},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    strategy::{
        harmony::r#trait::Harmony,
        melody::r#trait::{MelodyStrategy, MelodyStrategyAdaptor},
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    pub scales: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub initial_reference: Stack<T>,

    pub reanchor: bool,
    pub group_ms: u64,
}

/// The first three fields are exacly the same as for
/// [crate::strategy::staticneighbourhoods::StaticNeighbourhoods]
pub struct StaticNeighbourhoodsAsMelody<T: StackType> {
    /// This Vec must never be empty
    scales: Vec<SomeCompleteNeighbourhood<T>>,
    curr_scale_index: usize,

    reanchor: bool,

    last_solve: Instant,
    group_start_reference: Stack<T>,
    group_duration: Duration,

    tmp_stack: Stack<T>,
}

impl<T: StackType> IsMelodyStrategyConfig<T> for StaticNeighbourhoodsAsMelodyConfig<T> {
    fn as_melody_strategy_config(self) -> MelodyStrategyConfig<T> {
        MelodyStrategyConfig::StaticNeighbourhoods(self)
    }
}

impl<T: StackType> StaticNeighbourhoodsAsMelody<T> {
    fn tune_without_harmony(&mut self, time: Instant, adaptor: &impl MelodyStrategyAdaptor<T>) {
        adaptor.send(FromStrategy::CurrentHarmony {
            pattern_index: None {},
            reference: None {},
        });
        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                let mut the_tuning = adaptor.tuning_mut(i);
                self.tmp_stack.clone_from(&the_tuning.stack);
                self.scales[self.curr_scale_index].write_absolute_stack(
                    &mut the_tuning.stack,
                    i as StackCoeff,
                    &adaptor.reference(),
                );

                let mut retune = self.tmp_stack != the_tuning.stack;
                let new_semitones = the_tuning
                    .stack
                    .absolute_semitones(adaptor.tuning_reference().c4_semitones());
                if new_semitones != the_tuning.semitones {
                    the_tuning.semitones = new_semitones;
                    retune = true;
                }
                if retune {
                    adaptor.send(FromStrategy::Retune {
                        note: i as u8,
                        time,
                    });
                }
            }
        }
    }

    fn tune_with_valid_harmony(&mut self, time: Instant, adaptor: &impl MelodyStrategyAdaptor<T>) {
        if self.reanchor {
            todo!()
        } else {
            let Harmony {
                neighbourhood: harmony_neighbourhood,
                reference: harmony_reference,
                pattern_index,
                ..
            } = &*adaptor.harmony();
            adaptor.send(FromStrategy::CurrentHarmony {
                pattern_index: *pattern_index,
                reference: Some(
                    self.scales[self.curr_scale_index]
                        .get_absolute_stack(*harmony_reference, &adaptor.reference()),
                ),
            });
            for i in 0..128 {
                if adaptor.key_state(i).is_sounding() {
                    let mut the_tuning = adaptor.tuning_mut(i);
                    self.tmp_stack.clone_from(&the_tuning.stack);
                    if harmony_neighbourhood.try_write_relative_stack(
                        &mut the_tuning.stack,
                        i as StackCoeff - *harmony_reference,
                    ) {
                        self.scales[self.curr_scale_index].increment_by_absolute_stack(
                            &mut the_tuning.stack,
                            *harmony_reference,
                            &adaptor.reference(),
                        );
                    } else {
                        self.scales[self.curr_scale_index].write_absolute_stack(
                            &mut the_tuning.stack,
                            i as StackCoeff,
                            &adaptor.reference(),
                        );
                    }

                    let mut retune = self.tmp_stack != the_tuning.stack;
                    let new_semitones = the_tuning
                        .stack
                        .absolute_semitones(adaptor.tuning_reference().c4_semitones());
                    if new_semitones != the_tuning.semitones {
                        the_tuning.semitones = new_semitones;
                        retune = true;
                    }
                    if retune {
                        adaptor.send(FromStrategy::Retune {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
        }
    }

    fn update_all_tunings_and_send(
        &mut self,
        time: Instant,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) {
        if adaptor.harmony().valid {
            self.tune_with_valid_harmony(time, adaptor);
        } else {
            self.tune_without_harmony(time, adaptor);
        }
    }

    /// returns true iff the reference changed
    fn set_reference(
        &mut self,
        new_reference: Stack<T>,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) -> bool {
        if new_reference != *adaptor.reference() {
            adaptor.reference_mut().clone_from(&new_reference);
            adaptor.send(FromStrategy::UpdateReference {});
            true
        } else {
            false
        }
    }

    /// returns true iff the reference changed
    fn set_reference_to_current(&mut self, adaptor: &impl MelodyStrategyAdaptor<T>) -> bool {
        if adaptor.harmony().valid {
            self.scales[self.curr_scale_index].write_absolute_stack(
                &mut self.tmp_stack,
                adaptor.harmony().reference,
                &adaptor.reference(),
            );

            if *adaptor.reference() != self.tmp_stack {
                adaptor.reference_mut().clone_from(&self.tmp_stack);
                adaptor.send(FromStrategy::UpdateReference {});
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// returns true iff the reference changed
    fn set_reference_to_extreme(
        &mut self,
        to_highest: bool,
        adaptor: &impl MelodyStrategyAdaptor<T>,
    ) -> bool {
        self.tmp_stack.clone_from(&adaptor.reference());

        if to_highest {
            for i in (0..128).rev() {
                if adaptor.key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tuning(i).stack);
                    break;
                }
            }
        } else {
            for i in 0..128 {
                if adaptor.key_state(i).is_sounding() {
                    self.tmp_stack.clone_from(&adaptor.tuning(i).stack);
                    break;
                }
            }
        }

        if *adaptor.reference() != self.tmp_stack {
            adaptor.reference_mut().clone_from(&self.tmp_stack);
            adaptor.send(FromStrategy::UpdateReference {});
            true
        } else {
            false
        }
    }

    fn toggle_reanchor(&mut self, time: Instant) {
        self.reanchor = !self.reanchor;
        todo!()
    }
}

pub trait StaticNeighbourhoodsAsMelodyAdaptor<T: StackType>: MelodyStrategyAdaptor<T> {
    /// This function is allowed to be not extremely fast; it's only called in situations where we
    /// want to reload (parts of) the configuration.
    fn config(&self) -> impl Deref<Target = StaticNeighbourhoodsAsMelodyConfig<T>>;
}

impl<T: StackType, A: StaticNeighbourhoodsAsMelodyAdaptor<T>> MelodyStrategy<T, A>
    for StaticNeighbourhoodsAsMelody<T>
{
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(mut config: Self::Config) -> Self {
        Self {
            scales: config.scales.drain(..).map(|n| n.named).collect(),
            curr_scale_index: 0,
            reanchor: config.reanchor,
            last_solve: Instant::now(),
            group_start_reference: Stack::new_zero(),
            group_duration: Duration::from_millis(config.group_ms),
            tmp_stack: Stack::new_zero(),
        }
    }

    fn tune_with_harmony(&mut self, time: Instant, adaptor: &A) {
        self.update_all_tunings_and_send(time, adaptor);
    }

    fn stop(&mut self, _time: Instant, _adaptor: &A) {}

    fn start(&mut self, time: Instant, adaptor: &A) {
        adaptor.send(FromStrategy::UpdateReference {});
        adaptor.send(FromStrategy::SelectScale {
            index: self.curr_scale_index,
        });
        self.scales[self.curr_scale_index].for_each_stack(|_, stack| {
            let _ = adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });
        self.tune_with_harmony(time, adaptor);
    }

    fn reset(&mut self, adaptor: &A) {
        self.scales = adaptor
            .config()
            .scales
            .iter()
            .map(|n| n.named.clone())
            .collect();
        self.curr_scale_index = 0;
        adaptor
            .reference_mut()
            .clone_from(&adaptor.config().initial_reference);
        self.reanchor = adaptor.config().reanchor;
    }

    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A) {
        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                let new_semitones = adaptor
                    .tuning(i)
                    .stack
                    .absolute_semitones(adaptor.tuning_reference().c4_semitones());
                adaptor.tuning_mut(i).semitones = new_semitones;
                adaptor.send(FromStrategy::Retune {
                    note: i as u8,
                    time,
                });
            }
        }
    }

    fn consider(&mut self, stack: Stack<T>, time: Instant, adaptor: &A) {
        let inserted_stack = self.scales[self.curr_scale_index].insert(&stack).clone();
        let _ = adaptor.send(FromStrategy::Consider {
            stack: inserted_stack,
        });
        self.update_all_tunings_and_send(time, adaptor);
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) {
        match msg {
            ToStaticNeighbourhoodsAsMelody::SelectScale { index, time } => {
                if index != self.curr_scale_index {
                    self.curr_scale_index = index;
                    self.start(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::SetReference { reference, time } => {
                if self.set_reference(reference, adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            ToStaticNeighbourhoodsAsMelody::ToggleReanchor { time } => self.toggle_reanchor(time),
            ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms)
            }
            ToStaticNeighbourhoodsAsMelody::UpdateScales {
                only_this_scale,
                time,
            } => match only_this_scale {
                None {} => {
                    self.scales = adaptor
                        .config()
                        .scales
                        .iter()
                        .map(|n| n.named.clone())
                        .collect();
                    if self.scales.len() <= self.curr_scale_index {
                        self.curr_scale_index = 0;
                    }
                    self.start(time, adaptor);
                }
                Some(i) => {
                    self.scales[i].clone_from(&adaptor.config().scales[i].named);
                    if i == self.curr_scale_index {
                        self.start(time, adaptor);
                    }
                }
            },
        }
    }

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg> {
        match msg {
            ToMelody::StaticNeighbourhoods(msg) => Some(msg),
        }
    }

    // Make sure that [StrategyConfig::reacts_to_bound] exposes exactly the actions that this
    // function handles!
    fn handle_bound_action(&mut self, action: &BindableStrategyAction, time: Instant, adaptor: &A) {
        match action {
            BindableStrategyAction::IncrementNeighbourhoodIndex(increment) => {
                let old_index = self.curr_scale_index;
                self.curr_scale_index = (old_index as isize + increment)
                    .rem_euclid(self.scales.len() as isize)
                    as usize;
                if old_index != self.curr_scale_index {
                    self.start(time, adaptor);
                }
            }
            BindableStrategyAction::SetReferenceToLowest => {
                if self.set_reference_to_extreme(false, adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            BindableStrategyAction::SetReferenceToHighest => {
                if self.set_reference_to_extreme(true, adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            BindableStrategyAction::SetReferenceToCurrent => {
                if self.set_reference_to_current(adaptor) {
                    self.update_all_tunings_and_send(time, adaptor);
                }
            }
            BindableStrategyAction::Reset => {
                self.stop(time, adaptor);
                self.reset(adaptor);
                self.start(time, adaptor);
            }
            _ => {}
        }
    }
}
