use std::{marker::PhantomData, time::Instant};

use crate::{
    config::{IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig},
    interval::stacktype::r#trait::StackType,
    msg::{ToStrategy, ToTwoStep},
    strategy::{
        harmony::r#trait::{HarmonyStrategy, HarmonyStrategyAdaptor},
        melody::r#trait::{MelodyStrategy, MelodyStrategyAdaptor},
        r#trait::{Strategy, StrategyAdaptor},
    },
};

pub struct TwoStep<
    T: StackType,
    H: HarmonyStrategy<T, HA>,
    HA: HarmonyStrategyAdaptor<T>,
    M: MelodyStrategy<T, MA>,
    MA: MelodyStrategyAdaptor<T>,
> {
    _phantom: PhantomData<(T, HA, MA)>,

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

impl<T, H, HA, M, MA> TwoStep<T, H, HA, M, MA>
where
    T: StackType,
    H: HarmonyStrategy<T, HA>,
    HA: HarmonyStrategyAdaptor<T>,
    M: MelodyStrategy<T, MA>,
    MA: MelodyStrategyAdaptor<T>,
{
    fn start_solve(
        &mut self,
        time: Instant,
        adaptor: &impl TwoStepStrategyAdaptor<T, H, HA, M, MA>,
    ) -> bool {
        let res = self
            .harmony_strategy
            .start_solve(time, adaptor.as_harmony_adaptor());
        self.solve_start = time;
        self.solving_harmony = !res.finished;

        if res.progress | res.finished {
            self.melody_strategy
                .tune_with_harmony(self.solve_start, adaptor.as_melody_adaptor());
        }

        self.solving_harmony
    }
}

pub trait TwoStepStrategyAdaptor<T, H, HA, M, MA>: StrategyAdaptor<T>
where
    T: StackType,
    H: HarmonyStrategy<T, HA>,
    HA: HarmonyStrategyAdaptor<T>,
    M: MelodyStrategy<T, MA>,
    MA: MelodyStrategyAdaptor<T>,
{
    fn as_melody_adaptor(&self) -> &MA;
    fn as_harmony_adaptor(&self) -> &HA;
}

impl<T, H, HA, M, MA, A> Strategy<T, A> for TwoStep<T, H, HA, M, MA>
where
    T: StackType,
    H: HarmonyStrategy<T, HA>,
    HA: HarmonyStrategyAdaptor<T>,
    M: MelodyStrategy<T, MA>,
    MA: MelodyStrategyAdaptor<T>,
    A: TwoStepStrategyAdaptor<T, H, HA, M, MA>,
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

    fn start(&mut self, time: Instant, adaptor: &A) -> bool {
        let res = self
            .harmony_strategy
            .start(time, adaptor.as_harmony_adaptor());
        self.solving_harmony = !res.finished;
        self.melody_strategy
            .start(time, adaptor.as_melody_adaptor());
        self.solving_harmony
    }

    fn stop(&mut self, time: Instant, adaptor: &A) {
        self.harmony_strategy
            .stop(time, adaptor.as_harmony_adaptor());
        self.melody_strategy.stop(time, adaptor.as_melody_adaptor());
    }

    fn note_on(&mut self, _note: u8, time: Instant, adaptor: &A) -> bool {
        self.start_solve(time, adaptor)
    }

    fn note_off(&mut self, _note: u8, time: Instant, adaptor: &A) -> bool {
        self.start_solve(time, adaptor)
    }

    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A) -> bool {
        self.melody_strategy
            .update_tuning_reference(time, adaptor.as_melody_adaptor());
        self.solving_harmony
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) -> bool {
        match msg {
            ToTwoStep::ToHarmonyStrategy(msg) => {
                if let Some(x) = H::filter_to_harmony(msg) {
                    if let Some(time) = self
                        .harmony_strategy
                        .receive_msg(x, adaptor.as_harmony_adaptor())
                    {
                        return self.start_solve(time, adaptor);
                    }
                }
                self.solving_harmony
            }
            ToTwoStep::ToMelodyStrategy(msg) => {
                if let Some(x) = M::filter_to_melody(msg) {
                    self.melody_strategy
                        .receive_msg(x, adaptor.as_melody_adaptor());
                }
                self.solving_harmony
            }
        }
    }

    fn step(&mut self, adaptor: &A) -> bool {
        if self.solving_harmony {
            let res = self.harmony_strategy.step(adaptor.as_harmony_adaptor());

            self.solving_harmony = !res.finished;

            if res.progress | res.finished {
                self.melody_strategy
                    .tune_with_harmony(self.solve_start, adaptor.as_melody_adaptor());
            }
        }
        self.solving_harmony
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::TwoStep(msg) => Some(msg),
            _ => None {},
        }
    }
}
