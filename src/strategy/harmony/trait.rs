use std::{sync::mpsc, time::Instant};

use crate::{
    config::{ExtractConfig, IsHarmonyStrategyConfig},
    interval::stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    keystate::KeyState,
    msg::{ReceiveMsg, ToHarmony},
    neighbourhood::{Partial, SomeNeighbourhood},
    util::readerwriter::{Reader, ReaderWriter},
};

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: SomeNeighbourhood<T>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference: StackCoeff,
}

impl<T: IntervalBasis> Harmony<T> {
    pub fn new_dummy() -> Self {
        Self {
            neighbourhood: SomeNeighbourhood::Partial(Partial::new()),
            reference: 0,
        }
    }
}

/// The 'time' fields store the time of the event that triggered the solve.
pub enum HarmonyResult {
    StartSolve { time: Instant },
    StepWithProgress { time: Instant },
    StepNoProgress { time: Instant },
    FinishedWithResult { time: Instant },
    FinishedNoResult { time: Instant },
}

impl HarmonyResult {
    fn is_finished(&self) -> bool {
        match self {
            HarmonyResult::FinishedWithResult { .. } | HarmonyResult::FinishedNoResult { .. } => {
                true
            }
            _ => false,
        }
    }
}

pub trait HarmonyStrategy<T: StackType>:
    ReceiveMsg<Self::Msg> + ExtractConfig<Self::Config>
{
    type Config: IsHarmonyStrategyConfig<T, Realized = Self>;
    type Msg;

    fn new(config: Self::Config) -> Self;

    /// Starts solving a new configuration of keys. After this function, [HarmonyStrategy::step]
    /// will be called again and again, either until there's no more solutions, or the
    /// configuration of keys changes.
    fn start_solve(
        &mut self,
        time: Instant,
        keys: &Reader<[KeyState; 128]>,
        harmony: &ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult;

    /// Returns true iff a harmony was found that matches the currently pressed 'keys'. In that
    /// case the 'harmony' argument must contain that found harmony. Otherwise, its contents will
    /// be ignored.
    ///
    /// This function should never be called when a previous invocation of this function or of
    /// [HarmonyStrategy::start_solve] has returned any of the "Finished" results.
    fn step(
        &mut self,
        keys: &Reader<[KeyState; 128]>,
        harmony: &ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult;

    fn start(&mut self, time: Instant);
    fn stop(&mut self, time: Instant);

    fn filter_to_harmony(msg: ToHarmony<T>) -> Option<Self::Msg>;

    /// Should return the time of a [HarmonyStrategy::start_solve] that should be triggered by the
    /// message, if necessary.
    fn msg_requires_solve_at_time(msg: &Self::Msg) -> Option<Instant>;

    /// This is intended to run in its own thread.
    fn receive_solve_loop(
        &mut self,
        to_harmony_rx: mpsc::Receiver<ToHarmony<T>>,
        from_harmony_tx: mpsc::Sender<HarmonyResult>,
        keys: &Reader<[KeyState; 128]>,
        harmony: &ReaderWriter<Harmony<T>>,
    ) -> Self::Config {
        let mut last_msg = None {};
        let mut continue_solving = false;
        loop {
            if continue_solving {
                let res = self.step(keys, harmony);
                if res.is_finished() {
                    continue_solving = false;
                }
                let _ = from_harmony_tx.send(res);
                if let Ok(msg) = to_harmony_rx.try_recv() {
                    last_msg = Some(msg);
                }
            } else {
                match to_harmony_rx.recv() {
                    Ok(msg) => last_msg = Some(msg),
                    Err(_) => break,
                }
            }

            match last_msg.take() {
                None {} => {}
                Some(ToHarmony::Solve { time }) => {
                    let res = self.start_solve(time, &keys, &harmony);
                    continue_solving = !res.is_finished();
                    let _ = from_harmony_tx.send(res);
                }
                Some(ToHarmony::Stop { time }) => {
                    self.stop(time);
                    break;
                }
                Some(ToHarmony::Start { time }) => {
                    self.start(time);
                    continue_solving = false;
                }
                Some(msg) => {
                    if let Some(x) = Self::filter_to_harmony(msg) {
                        let time = Self::msg_requires_solve_at_time(&x);
                        self.receive_msg(x);
                        if let Some(t) = time {
                            let res = self.start_solve(t, &keys, &harmony);
                            continue_solving = !res.is_finished();
                            let _ = from_harmony_tx.send(res);
                        }
                    }
                }
            }
        }

        self.extract_config()
    }
}
