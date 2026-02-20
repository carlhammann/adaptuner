use std::time::Instant;

use crate::{
    config::{
        ExtractConfig, IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig,
        StrategyConfig,
    },
    interval::stacktype::r#trait::StackType,
    msg::{ToStrategy, ToTwoStep},
    reference::Reference,
    strategy::{
        harmony::r#trait::{Harmony, HarmonyStrategy},
        melody::r#trait::MelodyStrategy,
        r#trait::{Strategy, StrategyAdaptor},
    },
    util::readerwriter::ConcreteReaderWriter,
};

pub struct TwoStep<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> {
    harmony: ConcreteReaderWriter<Harmony<T>>,
    harmony_strategy: H,
    melody_strategy: M,
    solve_start: Instant,
    solving_harmony: bool,
    found_harmony: bool,
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>>
    ExtractConfig<(H::Config, M::Config)> for TwoStep<T, H, M>
{
    fn extract_config(&self) -> (H::Config, M::Config) {
        (
            self.harmony_strategy.extract_config(),
            self.melody_strategy.extract_config(),
        )
    }
}

impl<T, HC, MC> IsStrategyConfig<T> for (HC, MC)
where
    T: StackType + Send + Sync,
    HC: IsHarmonyStrategyConfig<T>,
    MC: IsMelodyStrategyConfig<T>,
{
    type Realized = TwoStep<T, HC::Realized, MC::Realized>;

    fn as_strategy_config(self) -> StrategyConfig<T> {
        StrategyConfig::TwoStep(
            self.0.as_harmony_strategy_config(),
            self.1.as_melody_strategy_config(),
        )
    }
}

impl<T, H, M> TwoStep<T, H, M>
where
    T: StackType,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
{
    fn start_solve(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        let res = self
            .harmony_strategy
            .start_solve(time, adaptor, &self.harmony);
        self.solve_start = time;
        self.solving_harmony = !res.finished;
        self.found_harmony = res.progress;

        if res.progress {
            self.melody_strategy
                .tune_with_harmony(self.solve_start, adaptor, &self.harmony, true);
        }

        if !self.found_harmony & res.finished {
            self.melody_strategy
                .tune_with_harmony(self.solve_start, adaptor, &self.harmony, false);
        }

        self.solving_harmony
    }
}

impl<T, H, M> Strategy<T> for TwoStep<T, H, M>
where
    T: StackType + Send + Sync,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
{
    type Msg = ToTwoStep<T>;

    type Config = (H::Config, M::Config);

    fn new(config: Self::Config) -> Self {
        Self {
            harmony: ConcreteReaderWriter::new(Harmony::new_dummy()),
            harmony_strategy: H::new(config.0),
            melody_strategy: M::new(config.1),
            solve_start: Instant::now(),
            solving_harmony: false,
            found_harmony: false,
        }
    }

    fn start(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        let res = self.harmony_strategy.start(time, adaptor, &self.harmony);
        self.solving_harmony = !res.finished;
        self.found_harmony = res.progress;
        self.melody_strategy
            .start(time, adaptor, &self.harmony, self.found_harmony);
        self.solving_harmony
    }

    fn stop(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) {
        self.harmony_strategy.stop(time, adaptor, &self.harmony);
        self.melody_strategy.stop(time, adaptor);
    }

    fn reset(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        todo!()
        // self.start(time, adaptor)
    }

    fn note_on(&mut self, _note: u8, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        self.start_solve(time, adaptor)
    }

    fn note_off(&mut self, _note: u8, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool {
        self.start_solve(time, adaptor)
    }

    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
    ) -> bool {
        self.melody_strategy.set_tuning_reference(
            reference,
            time,
            adaptor,
            &self.harmony,
            self.found_harmony,
        );
        self.solving_harmony
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &impl StrategyAdaptor<T>) -> bool {
        match msg {
            ToTwoStep::ToHarmonyStrategy(msg) => {
                if let Some(x) = H::filter_to_harmony(msg) {
                    if let Some(time) = self.harmony_strategy.receive_msg(x, adaptor, &self.harmony)
                    {
                        return self.start_solve(time, adaptor);
                    }
                }
                self.solving_harmony
            }
            ToTwoStep::ToMelodystrategy(msg) => {
                if let Some(x) = M::filter_to_melody(msg) {
                    self.melody_strategy
                        .receive_msg(x, adaptor, &self.harmony, self.found_harmony);
                }
                self.solving_harmony
            }
        }
    }

    fn step(&mut self, adaptor: &impl StrategyAdaptor<T>) -> bool {
        if self.solving_harmony {
            let res = self.harmony_strategy.step(adaptor, &self.harmony);

            self.solving_harmony = !res.finished;
            self.found_harmony |= res.progress;

            if res.progress {
                self.melody_strategy
                    .tune_with_harmony(self.solve_start, adaptor, &self.harmony, true);
            }

            if !self.found_harmony & res.finished {
                self.melody_strategy
                    .tune_with_harmony(self.solve_start, adaptor, &self.harmony, false);
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
