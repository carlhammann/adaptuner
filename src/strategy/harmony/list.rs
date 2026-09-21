use std::time::Instant;

use crate::{
    bindable::BindableStrategyAction,
    config::{HarmonyStrategyConfig, IsHarmonyStrategyConfig},
    interval::stacktype::r#trait::StackType,
    msg::ToHarmony,
    strategy::harmony::{
        chordlist::ChordList,
        r#trait::{HarmonyAdaptor, HarmonyResult, HarmonyStrategy},
        springs::HarmonySprings,
    },
    util::ordered_locks::{Nat, OrderedLocks, Zero},
};

enum SomeHarmonyStrategy<T: StackType> {
    ChordList(ChordList<T>),
    Springs(HarmonySprings<T>),
}

pub struct ListOfHarmonyStrategies<T: StackType> {
    /// Must contain each type of [SomeHarmonyStrategy] at most once.
    strategies: Vec<SomeHarmonyStrategy<T>>,
    currently_running: usize,
    solve_start: Instant,
}

impl<T: StackType> IsHarmonyStrategyConfig<T> for Vec<HarmonyStrategyConfig<T>> {
    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::List(self)
    }
}

trait AsListAdaptor<T: StackType, L: Nat> {
    fn as_list_adaptor(self) -> HarmonyAdaptor<T, ListOfHarmonyStrategies<T>, L>;
}

impl<T: StackType, L: Nat> AsListAdaptor<T, L> for HarmonyAdaptor<T, ChordList<T>, L> {
    fn as_list_adaptor(self) -> HarmonyAdaptor<T, ListOfHarmonyStrategies<T>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, L: Nat> AsListAdaptor<T, L> for HarmonyAdaptor<T, HarmonySprings<T>, L> {
    fn as_list_adaptor(self) -> HarmonyAdaptor<T, ListOfHarmonyStrategies<T>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

trait SpecializeAdaptor<T: StackType, L: Nat, S: HarmonyStrategy<T>> {
    fn specialize(self) -> HarmonyAdaptor<T, S, L>;
}

impl<T: StackType, L: Nat> SpecializeAdaptor<T, L, ChordList<T>>
    for HarmonyAdaptor<T, ListOfHarmonyStrategies<T>, L>
{
    fn specialize(self) -> HarmonyAdaptor<T, ChordList<T>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, L: Nat> SpecializeAdaptor<T, L, HarmonySprings<T>>
    for HarmonyAdaptor<T, ListOfHarmonyStrategies<T>, L>
{
    fn specialize(self) -> HarmonyAdaptor<T, HarmonySprings<T>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType> HarmonyStrategy<T> for ListOfHarmonyStrategies<T> {
    type Config = Vec<HarmonyStrategyConfig<T>>;

    type Msg = ToHarmony;

    fn new(mut config: Self::Config) -> Self {
        Self {
            strategies: config
                .drain(..)
                .map(|c| match c {
                    HarmonyStrategyConfig::ChordList(c) => {
                        SomeHarmonyStrategy::ChordList(ChordList::new(c))
                    }
                    HarmonyStrategyConfig::Springs(c) => {
                        SomeHarmonyStrategy::Springs(HarmonySprings::new(c))
                    }
                    HarmonyStrategyConfig::List(_) => {
                        panic!("Cannot have a nested list of harmony strategies")
                    }
                })
                .collect(),
            currently_running: 0,
            solve_start: Instant::now(),
        }
    }

    fn start_solve(
        &mut self,
        time: Instant,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        self.solve_start = time;
        self.currently_running = 0;
        let strat = &mut self.strategies[self.currently_running];
        (_, adaptor) = match strat {
            SomeHarmonyStrategy::Springs(strat) => {
                on_snd(strat.start_solve(time, adaptor.specialize()), |a| {
                    a.as_list_adaptor()
                })
            }
            SomeHarmonyStrategy::ChordList(strat) => {
                on_snd(strat.start_solve(time, adaptor.specialize()), |a| {
                    a.as_list_adaptor()
                })
            }
        };

        // the reason for this line: If the zeroth strategy finishes immediately because it is
        // disabled, we want to progress with the next strategy. [Self::step] implements this logic,
        // and I don't want to duplicate it here. One step is cheap.
        self.step(adaptor)
    }

    fn step(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        let strat = &mut self.strategies[self.currently_running];
        let mut res;
        (res, adaptor) = match strat {
            SomeHarmonyStrategy::Springs(strat) => {
                on_snd(strat.step(adaptor.specialize()), |a| a.as_list_adaptor())
            }
            SomeHarmonyStrategy::ChordList(strat) => {
                on_snd(strat.step(adaptor.specialize()), |a| a.as_list_adaptor())
            }
        };

        if res.finished && !res.perfect && (self.currently_running < self.strategies.len() - 1) {
            self.currently_running += 1;
            let strat = &mut self.strategies[self.currently_running];
            (res, adaptor) = match strat {
                SomeHarmonyStrategy::Springs(strat) => on_snd(
                    strat.start_solve(self.solve_start, adaptor.specialize()),
                    |a| a.as_list_adaptor(),
                ),
                SomeHarmonyStrategy::ChordList(strat) => on_snd(
                    strat.start_solve(self.solve_start, adaptor.specialize()),
                    |a| a.as_list_adaptor(),
                ),
            };
        }

        (res, adaptor)
    }

    #[inline]
    fn filter_to_harmony(msg: Self::Msg) -> Option<Self::Msg> {
        Some(msg)
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        let mut solve_time = None {};
        for strat in &mut self.strategies {
            let t;
            (t, adaptor) = match strat {
                SomeHarmonyStrategy::Springs(x) => {
                    if let Some(msg) = HarmonySprings::<T>::filter_to_harmony(msg.clone()) {
                        on_snd(x.receive_msg(msg, adaptor.specialize()), |a| {
                            a.as_list_adaptor()
                        })
                    } else {
                        (None {}, adaptor)
                    }
                }
                SomeHarmonyStrategy::ChordList(x) => {
                    if let Some(msg) = ChordList::<T>::filter_to_harmony(msg.clone()) {
                        on_snd(x.receive_msg(msg, adaptor.specialize()), |a| {
                            a.as_list_adaptor()
                        })
                    } else {
                        (None {}, adaptor)
                    }
                }
            };
            if solve_time.is_none() {
                solve_time = t;
            }
        }
        (solve_time, adaptor)
    }

    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        let mut solve_time = None {};
        for strat in &mut self.strategies {
            let t;
            (t, adaptor) = match strat {
                SomeHarmonyStrategy::Springs(x) => on_snd(
                    x.handle_bound_action(action, time, adaptor.specialize()),
                    |a| a.as_list_adaptor(),
                ),
                SomeHarmonyStrategy::ChordList(x) => on_snd(
                    x.handle_bound_action(action, time, adaptor.specialize()),
                    |a| a.as_list_adaptor(),
                ),
            };
            if solve_time.is_none() {
                solve_time = t;
            }
        }
        (solve_time, adaptor)
    }
}

#[inline]
fn on_snd<A, B, C>(x: (A, B), f: impl FnOnce(B) -> C) -> (A, C) {
    (x.0, f(x.1))
}
