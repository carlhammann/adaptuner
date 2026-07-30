use std::time::Instant;

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel,
        TuningStateLevel,
    },
    bindable::BindableStrategyAction,
    config::{IsStrategyConfig, Named, StrategyConfig},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{FromStrategy, ToStaticNeighbourhoods, ToStrategy},
    neighbourhood::{CompleteNeigbourhood, Neighbourhood, SomeCompleteNeighbourhood},
    process::r#trait::ProcessAdaptor,
    strategy::r#trait::{Strategy, StrategyAdaptor},
    util::ordered_locks::{AtMost, Succ, Zero},
};

pub struct StaticNeighbourhoods<T: StackType> {
    /// this Vec must never be empty
    scales: Vec<SomeCompleteNeighbourhood<T>>,
    curr_scale_index: usize,
    tmp_stack: Stack<T>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticNeighbourhoodsConfig<T: IntervalBasis> {
    /// this Vec must never be empty
    pub scales: Vec<Named<SomeCompleteNeighbourhood<T>>>,
    pub initial_reference: Stack<T>,
}

impl<T: StackType> StaticNeighbourhoods<T> {
    /// Only does something iff the tuning stack, as accessed through the adaptor, changes. That
    /// is: You can't use this for retunes caused by a changing tuning reference.
    fn update_tuning_and_send<P, L>(
        &mut self,
        note: u8,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, L>,
    ) -> StrategyAdaptor<T, Self, P, L>
    where
        P: ProcessAdaptor<StackType = T>,
        L: AtMost<TuningStateLevel>,
    {
        (_, adaptor) = adaptor.tuning_mut(note as usize, |the_tuning, mut adaptor| {
            (_, adaptor) = adaptor.reference(|reference, _| {
                self.scales[self.curr_scale_index].write_relative_stack(
                    &mut self.tmp_stack,
                    note as StackCoeff - reference.key_number(),
                );
                self.tmp_stack.scaled_add(1, reference);
            });

            let mut changed = false;

            if the_tuning.stack != self.tmp_stack {
                the_tuning.stack.clone_from(&self.tmp_stack);
                changed = true;
            }

            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let the_semitones = self.tmp_stack.absolute_semitones(c4_semitones);
            if the_semitones != the_tuning.semitones {
                the_tuning.semitones = the_semitones;
                changed = true;
            }

            if changed {
                adaptor.send(FromStrategy::Retune { note, time });
            }
        });

        adaptor
    }

    fn update_all_tunings_and_send<P, L>(
        &mut self,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, L>,
    ) -> StrategyAdaptor<T, Self, P, L>
    where
        P: ProcessAdaptor<StackType = T>,
        L: AtMost<KeyStateLevel>,
    {
        adaptor = adaptor.for_all_sounding_keys(|i, _, adaptor| {
            self.update_tuning_and_send(i as u8, time, adaptor);
        });
        adaptor
    }

    /// Returns true iff the reference changed. In that case, a re-tuning using
    /// [Self::update_all_tunings_and_send] will become necessary.
    fn set_reference_to_extreme<P, L>(
        &mut self,
        to_highest: bool,
        mut adaptor: StrategyAdaptor<T, Self, P, L>,
    ) -> (bool, StrategyAdaptor<T, Self, P, L>)
    where
        P: ProcessAdaptor<StackType = T>,
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        (_, adaptor) = adaptor.reference(|r, _| self.tmp_stack.clone_from(r));

        if to_highest {
            for i in (0..128).rev() {
                let mut found = false;
                (_, adaptor) = adaptor.key_state(i, |k, adaptor| {
                    if k.is_sounding() {
                        adaptor.tuning(i, |t, _| self.tmp_stack.clone_from(&t.stack));
                        found = true;
                    }
                });
                if found {
                    break;
                }
            }
        } else {
            for i in 0..128 {
                let mut found = false;
                (_, adaptor) = adaptor.key_state(i, |k, adaptor| {
                    if k.is_sounding() {
                        adaptor.tuning(i, |t, _| self.tmp_stack.clone_from(&t.stack));
                        found = true;
                    }
                });
                if found {
                    break;
                }
            }
        }

        adaptor.reference_mut(|old_reference, adaptor| {
            if *old_reference != self.tmp_stack {
                old_reference.clone_from(&self.tmp_stack);
                adaptor.send(FromStrategy::UpdateReference {});
                true
            } else {
                false
            }
        })
    }
}

impl<T: StackType> IsStrategyConfig<T> for StaticNeighbourhoodsConfig<T> {}

impl<T: StackType, P: ProcessAdaptor<StackType = T>, L: AtMost<StrategyConfigLevel>>
    StrategyAdaptor<T, StaticNeighbourhoods<T>, P, L>
{
    fn config<R>(
        self,
        mut f: impl FnMut(
            &StaticNeighbourhoodsConfig<T>,
            StrategyAdaptor<T, StaticNeighbourhoods<T>, P, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self) {
        self.active_strategy(|conf, adaptor| match conf {
            StrategyConfig::StaticNeighbourhoods { config, .. } => f(config, adaptor),
            _ => panic!("Wrong type of strategy config: expected StaticNeighbourhoodsConfig"),
        })
    }
}

impl<T: StackType> Strategy<T> for StaticNeighbourhoods<T> {
    type Msg = ToStaticNeighbourhoods<T>;
    type Config = StaticNeighbourhoodsConfig<T>;

    fn new(mut config: StaticNeighbourhoodsConfig<T>) -> Self {
        Self {
            scales: config.scales.drain(..).map(|n| n.named).collect(),
            curr_scale_index: 0,
            tmp_stack: Stack::new_zero(),
        }
    }

    fn start<P>(
        &mut self,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>)
    where
        P: ProcessAdaptor<StackType = T>,
    {
        adaptor.send(FromStrategy::UpdateReference {});

        adaptor.send(FromStrategy::SelectScale {
            index: self.curr_scale_index,
        });
        self.scales[self.curr_scale_index].for_each_stack(|_, stack| {
            let _ = adaptor.send(FromStrategy::Consider {
                stack: stack.clone(),
            });
        });

        adaptor = self.update_all_tunings_and_send(time, adaptor);

        (false, adaptor)
    }

    fn stop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        _time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero> {
        adaptor
    }

    fn reset<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero> {
        (_, adaptor) = adaptor.config(|config, adaptor| {
            self.scales = config.scales.iter().map(|n| n.named.clone()).collect();
            self.curr_scale_index = 0;
            adaptor.reference_mut(|reference, _| reference.clone_from(&config.initial_reference));
        });
        adaptor
    }

    fn note_on<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        note: u8,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        adaptor = self.update_tuning_and_send(note, time, adaptor);
        (false, adaptor)
    }

    fn note_off<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        _note: u8,
        _time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        (false, adaptor)
    }

    fn update_tuning_reference<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        adaptor = adaptor.for_all_sounding_tunings_mut(|i, tuning, mut adaptor| {
            let c4_semitones;
            (c4_semitones, adaptor) = adaptor.tuning_reference(|r, _| r.c4_semitones());
            let new_semitones = tuning.stack.absolute_semitones(c4_semitones);
            tuning.semitones = new_semitones;
            adaptor.send(FromStrategy::Retune {
                note: i as u8,
                time,
            });
        });
        (false, adaptor)
    }

    fn consider<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        let inserted_stack = self.scales[self.curr_scale_index].insert(&stack).clone();
        let _ = adaptor.send(FromStrategy::Consider {
            stack: inserted_stack,
        });
        adaptor = self.update_all_tunings_and_send(time, adaptor);
        (false, adaptor)
    }

    fn receive_msg<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        msg: Self::Msg,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        match msg {
            ToStaticNeighbourhoods::SelectScale { index, time } => {
                if index != self.curr_scale_index {
                    self.curr_scale_index = index;
                    (_, adaptor) = self.start(time, adaptor);
                }
            }
            ToStaticNeighbourhoods::UpdateScales {
                only_this_scale,
                time,
            } => match only_this_scale {
                None {} => {
                    (self.scales, adaptor) = adaptor
                        .config(|conf, _| conf.scales.iter().map(|n| n.named.clone()).collect());
                    if self.scales.len() <= self.curr_scale_index {
                        self.curr_scale_index = 0;
                    }
                    (_, adaptor) = self.start(time, adaptor);
                }
                Some(i) => {
                    (_, adaptor) =
                        adaptor.config(|conf, _| self.scales[i].clone_from(&conf.scales[i].named));
                    if i == self.curr_scale_index {
                        (_, adaptor) = self.start(time, adaptor);
                    }
                }
            },
            ToStaticNeighbourhoods::SetReference { reference, time } => {
                (_, adaptor) = adaptor.reference_mut(|r, _| r.clone_from(&reference));
                let _ = adaptor.send(FromStrategy::UpdateReference {});
                adaptor = self.update_all_tunings_and_send(time, adaptor);
            }
        }
        (false, adaptor)
    }

    fn step<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        // no steps are needed for anything.
        (false, adaptor)
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::StaticNeighbourhoods(msg) => Some(msg),
            _ => None {},
        }
    }

    // Make sure that [StrategyConfig::reacts_to_bound] exposes exactly the actions that this
    // function handles!
    fn handle_bound_action<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        match action {
            BindableStrategyAction::IncrementNeighbourhoodIndex(increment) => {
                let old_index = self.curr_scale_index;
                self.curr_scale_index = (old_index as isize + increment)
                    .rem_euclid(self.scales.len() as isize)
                    as usize;
                if old_index != self.curr_scale_index {
                    (_, adaptor) = self.start(time, adaptor);
                }
            }
            BindableStrategyAction::SetReferenceToLowest => {
                let update;
                (update, adaptor) = self.set_reference_to_extreme(false, adaptor);
                if update {
                    adaptor = self.update_all_tunings_and_send(time, adaptor);
                }
            }
            BindableStrategyAction::SetReferenceToHighest => {
                let update;
                (update, adaptor) = self.set_reference_to_extreme(true, adaptor);
                if update {
                    adaptor = self.update_all_tunings_and_send(time, adaptor);
                }
            }
            BindableStrategyAction::Reset => {
                adaptor = self.stop(time, adaptor);
                adaptor = self.reset(adaptor);
                (_, adaptor) = self.start(time, adaptor);
            }
            BindableStrategyAction::SetReferenceToCurrent => {}
            BindableStrategyAction::ToggleChordMatching => {}
            BindableStrategyAction::ToggleReanchor => {}
        }
        (false, adaptor)
    }
}
