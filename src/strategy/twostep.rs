use std::{marker::PhantomData, sync::mpsc, thread, time::Instant};

use crate::{
    config::{
        ExtractConfig, IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig,
        StrategyConfig,
    },
    interval::{
        stack::Stack,
        stacktype::r#trait::StackType,
    },
    keystate::KeyState,
    msg::{FromStrategy, ReceiveMsg, ToHarmony, ToMelody, ToStrategy, ToTwoStep},
    reference::Reference,
    strategy::{
        harmony::r#trait::{Harmony, HarmonyResult, HarmonyStrategy}, melody::r#trait::MelodyStrategy, r#trait::Strategy
    },
    util::readerwriter::{Reader, ReaderWriter},
};



pub struct TwoStep<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> {
    _phantom: PhantomData<(H, M)>,
    harmony_thread: thread::JoinHandle<H::Config>,
    to_harmony_tx: mpsc::Sender<ToHarmony<T>>,
    harmony: ReaderWriter<Harmony<T>>,
    _from_harmony_to_melody: thread::JoinHandle<()>,
    melody_thread: thread::JoinHandle<M::Config>,
    to_melody_tx: mpsc::Sender<ToMelody<T>>,
    key_states: Reader<[KeyState; 128]>,
    tunings: ReaderWriter<[Stack<T>; 128]>,
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> TwoStep<T, H, M> {
    fn send_to_harmony(&self, msg: ToHarmony<T>) {
        let _ = self.to_harmony_tx.send(msg);
    }

    fn send_to_melody(&self, msg: ToMelody<T>) {
        let _ = self.to_melody_tx.send(msg);
    }

    fn solve(&self, time: Instant) {
        // this triggers the melody_strategy as well, because the harmony_strategy will send
        // something after it has found a solution.
        self.send_to_harmony(ToHarmony::Solve { time });
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> ReceiveMsg<ToTwoStep<T>>
    for TwoStep<T, H, M>
{
    fn receive_msg(&mut self, msg: ToTwoStep<T>) {
        match msg {
            ToTwoStep::ToHarmonyStrategy(msg) => {
                self.send_to_harmony(msg);
            }
            ToTwoStep::ToMelodystrategy(msg) => {
                self.send_to_melody(msg);
            }
        }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>>
    ExtractConfig<(H::Config, M::Config)> for TwoStep<T, H, M>
{
    fn extract_config(&self) -> (H::Config, M::Config) {
        // let now = Instant::now();
        // self.send_to_melody(ToMelody::Stop { time: now });
        // self.send_to_harmony(ToHarmony::Stop { time: now });
        //
        // let melody_config = self.melody_thread.join();
        // let harmony_config = self.harmony_thread.join();

        todo!()
    }
}

impl<T, HC, MC> IsStrategyConfig<T> for (HC, MC)
where
    T: StackType + Send + Sync,
    HC: IsHarmonyStrategyConfig<T> + Send + 'static,
    MC: IsMelodyStrategyConfig<T> + Send + 'static,
{
    type Realized = TwoStep<T, HC::Realized, MC::Realized>;

    fn as_strategy_config(self) -> StrategyConfig<T> {
        StrategyConfig::TwoStep(
            self.0.as_harmony_strategy_config(),
            self.1.as_melody_strategy_config(),
        )
    }
}

impl<T, H, M> Strategy<T> for TwoStep<T, H, M>
where
    T: StackType + Send + Sync,
    H: HarmonyStrategy<T>,
    M: MelodyStrategy<T>,
    H::Config: Send + 'static,
    M::Config: Send + 'static,
{
    type Msg = ToTwoStep<T>;

    type Config = (H::Config, M::Config);

    fn new(config: Self::Config) -> Self {
        todo!()
    }

    fn start(&mut self, time: Instant, adaptor: &impl super::r#trait::StrategyAdaptor<T>) -> bool {
        todo!()
    }

    fn stop(&mut self, time: Instant, adaptor: &impl super::r#trait::StrategyAdaptor<T>) {
        todo!()
    }

    fn note_on(&mut self, note: u8, time: Instant, adaptor: &impl super::r#trait::StrategyAdaptor<T>) -> bool {
        todo!()
    }

    fn note_off(&mut self, note: u8, time: Instant, adaptor: &impl super::r#trait::StrategyAdaptor<T>) -> bool {
        todo!()
    }

    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl super::r#trait::StrategyAdaptor<T>,
    ) -> bool {
        todo!()
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &impl super::r#trait::StrategyAdaptor<T>) -> bool {
        todo!()
    }

    fn step(&mut self, adaptor: &impl super::r#trait::StrategyAdaptor<T>) -> bool {
        todo!()
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        todo!()
    }
}
//     type Msg = ToTwoStep<T>;
//     type Config = (H::Config, M::Config);
//
//     fn new(
//         config: (H::Config, M::Config),
//         forward: mpsc::Sender<FromStrategy<T>>,
//         key_states: Reader<[KeyState; 128]>,
//         tunings: ReaderWriter<[Stack<T>; 128]>,
//     ) -> Self {
//         let (harmony_config, melody_config) = config;
//
//         let harmony = ReaderWriter::new(Harmony::new_dummy());
//
//         let (to_harmony_tx, to_harmony_rx) = mpsc::channel();
//         let (from_harmony_tx, from_harmony_rx) = mpsc::channel();
//
//         let key_states_clone = key_states.clone();
//         let harmony_clone = harmony.clone();
//         let harmony_thread = thread::spawn(move || {
//             let mut harmony_strategy = H::new(harmony_config);
//             harmony_strategy.receive_solve_loop(
//                 to_harmony_rx,
//                 from_harmony_tx,
//                 &key_states_clone,
//                 &harmony_clone,
//             )
//         });
//
//         let (to_melody_tx, to_melody_rx) = mpsc::channel();
//         let to_melody_tx_clone = to_melody_tx.clone();
//
//         let from_harmony_to_melody = thread::spawn(move || loop {
//             match from_harmony_rx.recv() {
//                 Ok(HarmonyResult::StartSolve { .. }) => {}
//                 Ok(HarmonyResult::StepNoProgress { time })
//                 | Ok(HarmonyResult::FinishedNoResult { time }) => {
//                     let _ = to_melody_tx_clone.send(ToMelody::TuneNoHarmony { time });
//                 }
//                 Ok(HarmonyResult::StepWithProgress { time })
//                 | Ok(HarmonyResult::FinishedWithResult { time }) => {
//                     let _ = to_melody_tx_clone.send(ToMelody::TuneWithHarmony { time });
//                 }
//                 Err(_) => break,
//             }
//         });
//
//         let tunings_clone = tunings.clone();
//         let harmony_reader = harmony.clone().into_reader();
//         let melody_thread = thread::spawn(move || {
//             let mut melody_strategy = M::new(melody_config);
//             loop {
//                 match to_melody_rx.recv() {
//                     Ok(msg) => {
//                         let stop = melody_strategy.handle_to_melody(
//                             msg,
//                             &harmony_reader,
//                             &tunings_clone,
//                             &forward,
//                         );
//                         if stop {
//                             break;
//                         }
//                     }
//                     Err(_) => break,
//                 }
//             }
//             melody_strategy.extract_config()
//         });
//
//         Self {
//             _phantom: PhantomData,
//             harmony_thread,
//             to_harmony_tx,
//             harmony,
//             _from_harmony_to_melody: from_harmony_to_melody,
//             melody_thread,
//             to_melody_tx,
//             key_states,
//             tunings,
//         }
//     }
//
//     fn note_on(&mut self, _note: u8, time: Instant) {
//         self.solve(time);
//     }
//
//     fn note_off(&mut self, _note: u8, time: Instant) {
//         self.solve(time);
//     }
//
//     fn start(&mut self, time: Instant) {
//         self.send_to_harmony(ToHarmony::Start { time });
//         self.send_to_melody(ToMelody::Start { time });
//     }
//
//     fn stop(&mut self, time: Instant) {
//         self.send_to_harmony(ToHarmony::Stop { time });
//         self.send_to_melody(ToMelody::Stop { time });
//     }
//
//     fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant) {
//         self.send_to_melody(ToMelody::SetTuningReference { reference, time });
//     }
//
//     fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
//         match msg {
//             ToStrategy::TwoStep(msg) => Some(msg),
//             _ => None {},
//         }
//     }
// }
