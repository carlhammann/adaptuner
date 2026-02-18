use std::{sync::mpsc, time::Instant};

use crate::{
    config::{ExtractConfig, IsMelodyStrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{FromStrategy, HandleMsg, ToMelody},
    reference::Reference,
    strategy::harmony::r#trait::Harmony,
    util::readerwriter::{Reader, ReaderWriter},
};

pub trait MelodyStrategy<T: StackType>:
    HandleMsg<Self::Msg, FromStrategy<T>> + ExtractConfig<Self::Config>
{
    type Config: IsMelodyStrategyConfig<T, Realized = Self>;

    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelody::TuneWithHarmony].
    fn tune_with_harmony(
        &mut self,
        time: Instant,
        harmony: &Reader<Harmony<T>>,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    );

    /// Implementation of [ToMelody::TuneNoHarmony].
    fn tune_no_harmony(
        &mut self,
        time: Instant,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    );

    /// Implementation of [ToMelody::Stop]
    fn stop(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>);

    /// Implementation of [ToMelody::Start]
    fn start(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>);

    /// Implementation of [ToMelody::SetTuningReference]
    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant);

    fn filter_to_melody(msg: ToMelody<T>) -> Option<Self::Msg>;

    /// returns true iff the input message was [ToMelody::Stop]. In that case, you should stop
    /// receiving messages.
    fn handle_to_melody(
        &mut self,
        msg: ToMelody<T>,
        harmony: &Reader<Harmony<T>>,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    ) -> bool {
        match msg {
            ToMelody::Start { time } => self.start(time, forward),
            ToMelody::Stop { time } => {
                self.stop(time, forward);
                return true;
            }
            ToMelody::TuneWithHarmony { time } => {
                self.tune_with_harmony(time, harmony, tunings, forward)
            }
            ToMelody::TuneNoHarmony { time } => self.tune_no_harmony(time, tunings, forward),
            ToMelody::SetTuningReference { reference, time } => {
                self.set_tuning_reference(reference, time)
            }
            _ => {
                if let Some(x) = Self::filter_to_melody(msg) {
                    self.handle_msg(x, forward);
                }
            }
        }
        false
    }
}
