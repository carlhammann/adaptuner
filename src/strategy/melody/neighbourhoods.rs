use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{
    config::{ExtractConfig, IsMelodyStrategyConfig, MelodyStrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    msg::{FromStrategy, HandleMsg, ToMelody, ToStaticNeighbourhoodsAsMelody},
    reference::Reference,
    strategy::{
        harmony::r#trait::Harmony, melody::r#trait::MelodyStrategy, staticneighbourhoods::{StaticNeighbourhoods, StaticNeighbourhoodsConfig}
    },
    util::readerwriter::{Reader, ReaderWriter},
};

#[derive(Clone)]
pub struct StaticNeighbourhoodsAsMelodyConfig<T: IntervalBasis> {
    pub reanchor: bool,
    pub inner: StaticNeighbourhoodsConfig<T>,
    pub group_ms: u64,
}

pub struct StaticNeighbourhoodsAsMelody<T: StackType> {
    reanchor: bool,
    last_solve: Instant,
    group_start_reference: Stack<T>,
    group_duration: Duration,
    inner: StaticNeighbourhoods<T>,
}

impl<T: StackType> IsMelodyStrategyConfig<T> for StaticNeighbourhoodsAsMelodyConfig<T> {
    type Realized = StaticNeighbourhoodsAsMelody<T>;

    fn as_melody_strategy_config(self) -> MelodyStrategyConfig<T> {
        MelodyStrategyConfig::Neighbourhoods(self)
    }
}

impl<T: StackType> ExtractConfig<StaticNeighbourhoodsAsMelodyConfig<T>>
    for StaticNeighbourhoodsAsMelody<T>
{
    fn extract_config(&self) -> StaticNeighbourhoodsAsMelodyConfig<T> {
        StaticNeighbourhoodsAsMelodyConfig {
            reanchor: self.reanchor,
            inner: self.inner.extract_config(),
            group_ms: self.group_duration.as_millis() as u64,
        }
    }
}

impl<T: StackType> HandleMsg<ToStaticNeighbourhoodsAsMelody<T>, FromStrategy<T>>
    for StaticNeighbourhoodsAsMelody<T>
{
    fn handle_msg(
        &mut self,
        msg: ToStaticNeighbourhoodsAsMelody<T>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    ) {
        match msg {
            ToStaticNeighbourhoodsAsMelody::Basic(msg) => todo!(),
            ToStaticNeighbourhoodsAsMelody::SetReferenceToCurrent { time } => todo!(),
            ToStaticNeighbourhoodsAsMelody::ReanchorOnMatch { reanchor } => {
                self.reanchor = reanchor;
            }
            ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms } => {
                self.group_duration = Duration::from_millis(group_ms);
            }
        }
    }
}

impl<T: StackType> MelodyStrategy<T> for StaticNeighbourhoodsAsMelody<T> {
    type Config = StaticNeighbourhoodsAsMelodyConfig<T>;

    type Msg = ToStaticNeighbourhoodsAsMelody<T>;

    fn new(config: Self::Config) -> Self {
        todo!()
    }

    fn tune_with_harmony(
        &mut self,
        time: Instant,
        harmony: &Reader<Harmony<T>>,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    ) {
        todo!()
    }

    fn tune_no_harmony(
        &mut self,
        time: Instant,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    ) {
        todo!()
    }

    fn stop(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>) {
        todo!()
    }

    fn start(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>) {
        todo!()
    }

    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant) {
        todo!()
    }

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg> {
        todo!()
    }
}
