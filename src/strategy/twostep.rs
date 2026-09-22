use std::{
    marker::PhantomData,
    time::{Duration, Instant},
};

use crate::{
    adaptors::lock_levels::{ActiveStrategyIndexLevel, StrategyConfigLevel},
    bindable::BindableStrategyAction,
    config::{
        IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig,
        MelodyHarmonyCoordinationConfig, StrategyConfig,
    },
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{ToStrategy, ToTwoStep},
    strategy::{
        harmony::r#trait::{HarmonyAdaptor, HarmonyStrategy},
        melody::r#trait::{MelodyAdaptor, MelodyStrategy},
        r#trait::{Strategy, StrategyAdaptor},
    },
    util::ordered_locks::{AtMost, Nat, OrderedLocks, Succ, Zero},
};

pub struct TwoStep<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> {
    _phantom: PhantomData<T>,

    harmony_strategy: H,
    melody_strategy: M,

    solve_start: Instant,
    solving_harmony: bool,

    group_start: Instant,
    group_start_reference: Stack<T>,
    last_tune: Instant,
}

impl<T, HC, MC> IsStrategyConfig<T> for (HC, MC)
where
    T: StackType,
    HC: IsHarmonyStrategyConfig<T>,
    MC: IsMelodyStrategyConfig<T>,
{
}

trait AsMelodyAdaptor<T: StackType, M: MelodyStrategy<T>, L: Nat> {
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, L>;
}

trait AsHarmonyAdaptor<T: StackType, H: HarmonyStrategy<T>, L: Nat> {
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, L>;
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsHarmonyAdaptor<T, H, L>
    for StrategyAdaptor<T, TwoStep<T, H, M>, L>
{
    #[inline]
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsHarmonyAdaptor<T, H, L>
    for MelodyAdaptor<T, M, L>
{
    #[inline]
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsMelodyAdaptor<T, M, L>
    for StrategyAdaptor<T, TwoStep<T, H, M>, L>
{
    #[inline]
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsMelodyAdaptor<T, M, L>
    for HarmonyAdaptor<T, H, L>
{
    #[inline]
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

trait AsTwoStepAdaptor<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> {
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, L>;
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsTwoStepAdaptor<T, H, M, L>
    for MelodyAdaptor<T, M, L>
{
    #[inline]
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: Nat> AsTwoStepAdaptor<T, H, M, L>
    for HarmonyAdaptor<T, H, L>
{
    #[inline]
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>, L: AtMost<StrategyConfigLevel>>
    StrategyAdaptor<T, TwoStep<T, H, M>, L>
{
    fn melody_harmony_coordination<R>(
        self,
        mut f: impl FnMut(
            &MelodyHarmonyCoordinationConfig,
            StrategyAdaptor<T, TwoStep<T, H, M>, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self) {
        self.active_strategy(|conf, adaptor| match conf {
            StrategyConfig::TwoStep {
                melody_harmony_coordination,
                ..
            } => f(melody_harmony_coordination, adaptor),
            _ => panic!("Wrong type of strategy config: expected TwoStepConfig"),
        })
    }
}

impl<T, H, M> TwoStep<T, H, M>
where
    T: StackType,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
{
    #[inline]
    fn finish_solve(
        &mut self,
        finished: bool,
        mut adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> StrategyAdaptor<T, Self, Zero> {
        let now = Instant::now();
        let tune_wait_duration;
        (tune_wait_duration, adaptor) = adaptor.melody_harmony_coordination(
            |MelodyHarmonyCoordinationConfig { tune_wait_us, .. }, _| {
                Duration::from_micros(*tune_wait_us)
            },
        );
        if finished || (now.duration_since(self.last_tune) > tune_wait_duration) {
            let reanchor;
            let group_ms;
            ((reanchor, group_ms), adaptor) = adaptor.melody_harmony_coordination(
                |MelodyHarmonyCoordinationConfig {
                     reanchor, group_ms, ..
                 },
                 _| (*reanchor, *group_ms),
            );
            if reanchor
                && (self
                    .solve_start
                    .duration_since(self.group_start)
                    .as_millis()
                    <= group_ms as u128)
            {
                (_, adaptor) = adaptor.reference_mut(|reference, _| {
                    reference.clone_from(&self.group_start_reference)
                });
            }

            let mut ma = self
                .melody_strategy
                .tune_with_harmony(self.solve_start, adaptor.as_melody_adaptor());
            self.last_tune = Instant::now();

            if reanchor {
                ma = self.melody_strategy.handle_bound_action(
                    &BindableStrategyAction::SetReferenceToCurrent,
                    self.solve_start,
                    ma,
                );
            }
            ma.as_two_step_adaptor()
        } else {
            adaptor
        }
    }

    #[inline]
    fn start_solve(
        &mut self,
        time: Instant,
        mut adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        let group_ms;
        (group_ms, adaptor) = adaptor.melody_harmony_coordination(
            |MelodyHarmonyCoordinationConfig { group_ms, .. }, _| *group_ms,
        );
        if time.duration_since(self.group_start).as_millis() > group_ms as u128 {
            self.group_start = time;
            (_, adaptor) =
                adaptor.reference(|reference, _| self.group_start_reference.clone_from(reference));
        }

        let (res, ha) = self
            .harmony_strategy
            .start_solve(time, adaptor.as_harmony_adaptor());
        self.solve_start = time;
        self.solving_harmony = !res.finished;

        if res.progress || res.finished {
            adaptor = self.finish_solve(res.finished, ha.as_two_step_adaptor());
            (self.solving_harmony, adaptor)
        } else {
            (self.solving_harmony, ha.as_two_step_adaptor())
        }
    }
}

impl<T, H, M> Strategy<T> for TwoStep<T, H, M>
where
    T: StackType,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
{
    type Msg = ToTwoStep<T>;

    type Config = (H::Config, M::Config);

    fn new(config: Self::Config) -> Self {
        Self {
            _phantom: PhantomData,
            group_start_reference: match config.1.initial_scale_reference() {
                Some(r) => r.clone(),
                None {} => Stack::new_zero(),
            },
            harmony_strategy: H::new(config.0),
            melody_strategy: M::new(config.1),
            solve_start: Instant::now(),
            solving_harmony: false,
            group_start: Instant::now(),
            last_tune: Instant::now(),
        }
    }

    fn start(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        let (res, ha) = self
            .harmony_strategy
            .start_solve(time, adaptor.as_harmony_adaptor());
        self.solving_harmony = !res.finished;
        let ma = self.melody_strategy.start(time, ha.as_melody_adaptor());
        (self.solving_harmony, ma.as_two_step_adaptor())
    }

    fn stop(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> StrategyAdaptor<T, Self, Zero> {
        self.melody_strategy
            .stop(time, adaptor.as_melody_adaptor())
            .as_two_step_adaptor()
    }

    fn note_on(
        &mut self,
        _note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        self.start_solve(time, adaptor)
    }

    fn note_off(
        &mut self,
        _note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        self.start_solve(time, adaptor)
    }

    fn update_tuning_reference(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        let ma = self
            .melody_strategy
            .update_tuning_reference(time, adaptor.as_melody_adaptor());
        (self.solving_harmony, ma.as_two_step_adaptor())
    }

    fn consider(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        let ma = self
            .melody_strategy
            .consider(stack, time, adaptor.as_melody_adaptor());
        (false, ma.as_two_step_adaptor())
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        match msg {
            ToTwoStep::ToHarmonyStrategy(msg) => {
                if let Some(x) = H::filter_to_harmony(msg) {
                    let (mtime, ha) = self
                        .harmony_strategy
                        .receive_msg(x, adaptor.as_harmony_adaptor());
                    if let Some(time) = mtime {
                        self.start_solve(time, ha.as_two_step_adaptor())
                    } else {
                        (self.solving_harmony, ha.as_two_step_adaptor())
                    }
                } else {
                    (self.solving_harmony, adaptor)
                }
            }
            ToTwoStep::ToMelodyStrategy(msg) => {
                if let Some(x) = M::filter_to_melody(msg) {
                    let ma = self
                        .melody_strategy
                        .receive_msg(x, adaptor.as_melody_adaptor());
                    (self.solving_harmony, ma.as_two_step_adaptor())
                } else {
                    (self.solving_harmony, adaptor)
                }
            }
        }
    }

    fn step(
        &mut self,
        mut adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        if self.solving_harmony {
            let (res, ha) = self.harmony_strategy.step(adaptor.as_harmony_adaptor());

            self.solving_harmony = !res.finished;

            if res.progress || res.finished {
                adaptor = self.finish_solve(res.finished, ha.as_two_step_adaptor());
                (self.solving_harmony, adaptor)
            } else {
                (self.solving_harmony, ha.as_two_step_adaptor())
            }
        } else {
            (false, adaptor)
        }
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::TwoStep(msg) => Some(msg),
            _ => None {},
        }
    }

    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        let ma =
            self.melody_strategy
                .handle_bound_action(&action, time, adaptor.as_melody_adaptor());

        let (mtime, ha) =
            self.harmony_strategy
                .handle_bound_action(action, time, ma.as_harmony_adaptor());

        if let Some(solve_time) = mtime {
            self.start_solve(solve_time, ha.as_two_step_adaptor())
        } else {
            (self.solving_harmony, ha.as_two_step_adaptor())
        }
    }
}
