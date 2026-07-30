use std::{marker::PhantomData, time::Instant};

use crate::{
    bindable::BindableStrategyAction,
    config::{IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{ToStrategy, ToTwoStep},
    process::r#trait::ProcessAdaptor,
    strategy::{
        harmony::r#trait::{HarmonyAdaptor, HarmonyStrategy},
        melody::r#trait::{MelodyAdaptor, MelodyStrategy},
        r#trait::{Strategy, StrategyAdaptor},
    },
    util::ordered_locks::{Nat, OrderedLocks, Zero},
};

pub struct TwoStep<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> {
    _phantom: PhantomData<T>,

    harmony_strategy: H,
    melody_strategy: M,

    solve_start: Instant,
    solving_harmony: bool,
}

impl<T, HC, MC> IsStrategyConfig<T> for (HC, MC)
where
    T: StackType,
    HC: IsHarmonyStrategyConfig<T>,
    MC: IsMelodyStrategyConfig<T>,
{
}

trait AsMelodyAdaptor<T: StackType, M: MelodyStrategy<T>, P: ProcessAdaptor<StackType = T>, L: Nat>
{
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, P, L>;
}

trait AsHarmonyAdaptor<
    T: StackType,
    H: HarmonyStrategy<T>,
    P: ProcessAdaptor<StackType = T>,
    L: Nat,
>
{
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, P, L>;
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsHarmonyAdaptor<T, H, P, L> for StrategyAdaptor<T, TwoStep<T, H, M>, P, L>
{
    #[inline]
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsHarmonyAdaptor<T, H, P, L> for MelodyAdaptor<T, M, P, L>
{
    #[inline]
    fn as_harmony_adaptor(self) -> HarmonyAdaptor<T, H, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsMelodyAdaptor<T, M, P, L> for StrategyAdaptor<T, TwoStep<T, H, M>, P, L>
{
    #[inline]
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsMelodyAdaptor<T, M, P, L> for HarmonyAdaptor<T, H, P, L>
{
    #[inline]
    fn as_melody_adaptor(self) -> MelodyAdaptor<T, M, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

trait AsTwoStepAdaptor<
    T: StackType,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
    P: ProcessAdaptor<StackType = T>,
    L: Nat,
>
{
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, P, L>;
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsTwoStepAdaptor<T, H, M, P, L> for MelodyAdaptor<T, M, P, L>
{
    #[inline]
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<
        T: StackType,
        H: HarmonyStrategy<T>,
        M: MelodyStrategy<T>,
        P: ProcessAdaptor<StackType = T>,
        L: Nat,
    > AsTwoStepAdaptor<T, H, M, P, L> for HarmonyAdaptor<T, H, P, L>
{
    #[inline]
    fn as_two_step_adaptor(self) -> StrategyAdaptor<T, TwoStep<T, H, M>, P, L> {
        unsafe { OrderedLocks::new(self.inner_arc()) }
    }
}

impl<T, H, M> TwoStep<T, H, M>
where
    T: StackType,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
{
    fn start_solve<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        let (res, ha) = self
            .harmony_strategy
            .start_solve(time, adaptor.as_harmony_adaptor());
        self.solve_start = time;
        self.solving_harmony = !res.finished;

        if res.progress | res.finished {
            let ma = self
                .melody_strategy
                .tune_with_harmony(self.solve_start, ha.as_melody_adaptor());
            (self.solving_harmony, ma.as_two_step_adaptor())
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
            harmony_strategy: H::new(config.0),
            melody_strategy: M::new(config.1),
            solve_start: Instant::now(),
            solving_harmony: false,
        }
    }

    fn start<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        let (res, ha) = self
            .harmony_strategy
            .start(time, adaptor.as_harmony_adaptor());
        self.solving_harmony = !res.finished;
        let ma = self.melody_strategy.start(time, ha.as_melody_adaptor());
        (self.solving_harmony, ma.as_two_step_adaptor())
    }

    fn stop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero> {
        let ha = self
            .harmony_strategy
            .stop(time, adaptor.as_harmony_adaptor());
        self.melody_strategy
            .stop(time, ha.as_melody_adaptor())
            .as_two_step_adaptor()
    }

    fn reset<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero> {
        let ha = self.harmony_strategy.reset(adaptor.as_harmony_adaptor());
        self.melody_strategy
            .reset(ha.as_melody_adaptor())
            .as_two_step_adaptor()
    }

    fn note_on<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        self.start_solve(time, adaptor)
    }

    fn note_off<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        self.start_solve(time, adaptor)
    }

    fn update_tuning_reference<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        let ma = self
            .melody_strategy
            .update_tuning_reference(time, adaptor.as_melody_adaptor());
        (self.solving_harmony, ma.as_two_step_adaptor())
    }

    fn consider<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        let ma = self
            .melody_strategy
            .consider(stack, time, adaptor.as_melody_adaptor());
        (false, ma.as_two_step_adaptor())
    }

    fn receive_msg<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        msg: Self::Msg,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
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

    fn step<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
        if self.solving_harmony {
            let (res, ha) = self.harmony_strategy.step(adaptor.as_harmony_adaptor());

            self.solving_harmony = !res.finished;

            if res.progress | res.finished {
                let ma = self
                    .melody_strategy
                    .tune_with_harmony(self.solve_start, ha.as_melody_adaptor());
                (self.solving_harmony, ma.as_two_step_adaptor())
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

    fn handle_bound_action<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>) {
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
