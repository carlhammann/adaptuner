use std::{marker::PhantomData, sync::mpsc, thread, time::Instant};

use crate::{
    config::{
        ExtractConfig, IsHarmonyStrategyConfig, IsMelodyStrategyConfig, IsStrategyConfig,
        StrategyConfig,
    },
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, HandleMsg, ReceiveMsg, ToHarmony, ToMelody, ToStrategy, ToTwoStep},
    neighbourhood::SomeNeighbourhood,
    reference::Reference,
    strategy::r#trait::Strategy,
    util::readerwriter::{Reader, ReaderWriter},
};

pub mod harmony;
pub mod melody;

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: SomeNeighbourhood<T>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference: StackCoeff,
}

impl<T: IntervalBasis> Harmony<T> {
    fn new_dummy() -> Self {
        todo!()
    }
}

enum ToHarmonyWrapped<T: StackType> {
    /// You should possible send a [ToHarmonyWrapped::Solve] after this, depending on whether the
    /// message should trigger a re-adjustment of tunings.
    Msg(ToHarmony<T>, Instant),
    Start {
        time: Instant,
    },
    Stop {
        time: Instant,
    },
    Solve {
        time: Instant,
    },
}

enum FromHarmonyWrapped {
    NewSolve { time: Instant },
    FoundHarmony { time: Instant },
    NoHarmony { time: Instant },
}

pub trait HarmonyStrategy<T: StackType>:
    ReceiveMsg<ToHarmony<T>> + ExtractConfig<Self::Config>
{
    type Config: IsHarmonyStrategyConfig<T, Realized = Self>;

    fn new(config: Self::Config) -> Self;

    /// Starts solving a new configuration of keys. After this function, [HarmonyStrategy::step]
    /// will be called again and again, either until there's no more solutions, or the
    /// configuration of keys changes.
    fn new_solve(
        &mut self,
        time: Instant,
        keys: &Reader<[KeyState; 128]>,
        harmony: &ReaderWriter<Harmony<T>>,
    );

    /// Returns true iff a harmony was found that matches the currently pressed 'keys'. In that
    /// case the 'harmony' argument must contain that found harmony. Otherwise, its contents will
    /// be ignored.
    fn step(&mut self, keys: &Reader<[KeyState; 128]>, harmony: &ReaderWriter<Harmony<T>>) -> bool;

    /// Returns true iff there are no more approaches to try tuning the current keys.
    fn is_finished(&self) -> bool;

    fn start(&mut self, time: Instant);
    fn stop(&mut self, time: Instant);
}

enum ToMelodyWrapped<T: StackType> {
    Msg(ToMelody<T>, Instant),
    Start {
        time: Instant,
    },
    Stop {
        time: Instant,
    },
    TuneWithHarmony {
        time: Instant,
    },
    TuneNoHarmony {
        time: Instant,
    },
    SetTuningReference {
        reference: Reference<T>,
        time: Instant,
    },
}

pub trait MelodyStrategy<T: StackType>:
    HandleMsg<ToMelody<T>, FromStrategy<T>> + ExtractConfig<Self::Config>
{
    type Config: IsMelodyStrategyConfig<T, Realized = Self>;

    fn new(config: Self::Config) -> Self;

    /// Implementation of [ToMelodyWrapped::TuneWithHarmony].
    fn tune_with_harmony(
        &mut self,
        harmony: &Reader<Harmony<T>>,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    );

    /// Implementation of [ToMelodyWrapped::TuneNoHarmony].
    fn tune_no_harmony(
        &mut self,
        tunings: &ReaderWriter<[Stack<T>; 128]>,
        forward: &mpsc::Sender<FromStrategy<T>>,
    );

    /// Implementation of [ToMelodyWrapped::Stop]
    fn stop(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>);

    /// Implementation of [ToMelodyWrapped::Start]
    fn start(&mut self, time: Instant, forward: &mpsc::Sender<FromStrategy<T>>);

    /// Implementation of [ToMelodyWrapped::SetTuningReference]
    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant);
}

pub struct TwoStep<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> {
    _phantom: PhantomData<(H, M)>,
    harmony_thread: thread::JoinHandle<H::Config>,
    to_harmony_tx: mpsc::Sender<ToHarmonyWrapped<T>>,
    harmony: ReaderWriter<Harmony<T>>,
    _from_harmony_to_melody: thread::JoinHandle<()>,
    melody_thread: thread::JoinHandle<M::Config>,
    to_melody_tx: mpsc::Sender<ToMelodyWrapped<T>>,
    key_states: Reader<[KeyState; 128]>,
    tunings: ReaderWriter<[Stack<T>; 128]>,
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> TwoStep<T, H, M> {
    fn send_to_harmony(&self, msg: ToHarmonyWrapped<T>) {
        let _ = self.to_harmony_tx.send(msg);
    }

    fn send_to_melody(&self, msg: ToMelodyWrapped<T>) {
        let _ = self.to_melody_tx.send(msg);
    }

    fn solve(&self, time: Instant) {
        // this triggers the melody_strategy as well, because the harmony_strategy will send
        // something after it has found a solution.
        self.send_to_harmony(ToHarmonyWrapped::Solve { time });
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>> ReceiveMsg<ToTwoStep<T>>
    for TwoStep<T, H, M>
{
    fn receive_msg(&mut self, msg: ToTwoStep<T>) {
        match msg {
            ToTwoStep::ToHarmonyStrategy(msg, time) => {
                self.send_to_harmony(ToHarmonyWrapped::Msg(msg, time));
                self.solve(time);
            }
            ToTwoStep::ToMelodystrategy(msg, time) => {
                self.send_to_melody(ToMelodyWrapped::Msg(msg, time));
                self.solve(time);
            }
        }
    }
}

impl<T: StackType, H: HarmonyStrategy<T>, M: MelodyStrategy<T>>
    ExtractConfig<(H::Config, M::Config)> for TwoStep<T, H, M>
{
    fn extract_config(&self) -> (H::Config, M::Config) {
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

    fn new(
        config: (H::Config, M::Config),
        forward: mpsc::Sender<FromStrategy<T>>,
        key_states: Reader<[KeyState; 128]>,
        tunings: ReaderWriter<[Stack<T>; 128]>,
    ) -> Self {
        let (harmony_config, melody_config) = config;

        let harmony = ReaderWriter::new(Harmony::new_dummy());

        let (to_harmony_tx, to_harmony_rx) = mpsc::channel();
        let (from_harmony_tx, from_harmony_rx) = mpsc::channel();

        let key_states_clone = key_states.clone();
        let harmony_clone = harmony.clone();
        let harmony_thread = thread::spawn(move || {
            let mut harmony_strategy = H::new(harmony_config);

            let mut last_msg = None {};
            loop {
                if last_msg.is_none() {
                    match to_harmony_rx.recv() {
                        Ok(msg) => last_msg = Some(msg),
                        Err(_) => break,
                    }
                }

                match last_msg.take() {
                    Some(ToHarmonyWrapped::Solve { time }) => {
                        harmony_strategy.new_solve(time, &key_states_clone, &harmony_clone);
                        let _ = from_harmony_tx.send(FromHarmonyWrapped::NewSolve { time });

                        last_msg = None {};
                        while !harmony_strategy.is_finished() & last_msg.is_none() {
                            if harmony_strategy.step(&key_states_clone, &harmony_clone) {
                                let _ =
                                    from_harmony_tx.send(FromHarmonyWrapped::FoundHarmony { time });
                            } else {
                                let _ =
                                    from_harmony_tx.send(FromHarmonyWrapped::NoHarmony { time });
                            }
                            if let Ok(msg) = to_harmony_rx.try_recv() {
                                last_msg = Some(msg);
                            }
                        }
                    }
                    Some(ToHarmonyWrapped::Stop { time }) => {
                        harmony_strategy.stop(time);
                        break;
                    }
                    Some(ToHarmonyWrapped::Start { time }) => {
                        harmony_strategy.start(time);
                    }
                    Some(ToHarmonyWrapped::Msg(msg, time)) => {
                        harmony_strategy.receive_msg(msg);
                    }
                    None {} => unreachable!(),
                }
            }

            harmony_strategy.extract_config()
        });

        let (to_melody_tx, to_melody_rx) = mpsc::channel();
        let to_melody_tx_clone = to_melody_tx.clone();

        let from_harmony_to_melody = thread::spawn(move || loop {
            match from_harmony_rx.recv() {
                Ok(FromHarmonyWrapped::NewSolve { time }) => todo!(),
                Ok(FromHarmonyWrapped::NoHarmony { time }) => {
                    let _ = to_melody_tx_clone.send(ToMelodyWrapped::TuneNoHarmony { time });
                }
                Ok(FromHarmonyWrapped::FoundHarmony { time }) => {
                    let _ = to_melody_tx_clone.send(ToMelodyWrapped::TuneWithHarmony { time });
                }
                Err(_) => break,
            }
        });

        let tunings_clone = tunings.clone();
        let harmony_reader = harmony.clone().into_reader();
        let melody_thread = thread::spawn(move || {
            let mut melody_strategy = M::new(melody_config);
            loop {
                match to_melody_rx.recv() {
                    Ok(ToMelodyWrapped::TuneNoHarmony { time }) => {
                        melody_strategy.tune_no_harmony(&tunings_clone, &forward)
                    }
                    Ok(ToMelodyWrapped::TuneWithHarmony { time }) => {
                        melody_strategy.tune_with_harmony(&harmony_reader, &tunings_clone, &forward)
                    }
                    Ok(ToMelodyWrapped::SetTuningReference { reference, time }) => {
                        melody_strategy.set_tuning_reference(reference, time);
                    }
                    Ok(ToMelodyWrapped::Stop { time }) => {
                        melody_strategy.stop(time, &forward);
                        break;
                    }
                    Ok(ToMelodyWrapped::Start { time }) => {
                        melody_strategy.start(time, &forward);
                    }
                    Ok(ToMelodyWrapped::Msg(msg, _)) => melody_strategy.handle_msg(msg, &forward),
                    Err(_) => break,
                }
            }
            melody_strategy.extract_config()
        });

        Self {
            _phantom: PhantomData,
            harmony_thread,
            to_harmony_tx,
            harmony,
            _from_harmony_to_melody: from_harmony_to_melody,
            melody_thread,
            to_melody_tx,
            key_states,
            tunings,
        }
    }

    fn note_on(&mut self, _note: u8, time: Instant) {
        self.solve(time);
    }

    fn note_off(&mut self, _note: u8, time: Instant) {
        self.solve(time);
    }

    fn start(&mut self, time: Instant) {
        self.send_to_harmony(ToHarmonyWrapped::Start { time });
        self.send_to_melody(ToMelodyWrapped::Start { time });
    }

    fn stop(&mut self, time: Instant) {
        self.send_to_harmony(ToHarmonyWrapped::Stop { time });
        self.send_to_melody(ToMelodyWrapped::Stop { time });
    }

    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant) {
        todo!()
    }

    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg> {
        match msg {
            ToStrategy::TwoStep(msg) => Some(msg),
            _ => None {},
        }
    }
}
