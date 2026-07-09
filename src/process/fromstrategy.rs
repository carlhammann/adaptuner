use std::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::mpsc,
    sync::Arc,
    thread,
    time::Instant,
};

use midi_msg::{Channel, ChannelVoiceMsg::*, ControlChange::Hold, MidiMsg};
use parking_lot::RwLock;

use crate::{
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, ReceiveMsg, ToProcess, ToStrategy},
    process::r#trait::{ProcessAdaptor, StackWithTuning},
    reference::Reference,
    strategy::{
        harmony::{
            chordlist::{ChordList, ChordListAdaptor, ChordListConfig},
            r#trait::{Harmony, HarmonyStrategyAdaptor},
        },
        melody::{
            neighbourhoods::{
                StaticNeighbourhoodsAsMelody, StaticNeighbourhoodsAsMelodyAdaptor,
                StaticNeighbourhoodsAsMelodyConfig,
            },
            r#trait::MelodyStrategyAdaptor,
        },
        r#trait::{Strategy, StrategyAdaptor},
        staticneighbourhoods::{
            StaticNeighbourhoods, StaticNeighbourhoodsAdaptor, StaticNeighbourhoodsConfig,
        },
        twostep::{TwoStep, TwoStepStrategyAdaptor},
    },
    util::mapderefmut::MapDerefMut,
};

struct RunningStrategy<T: StackType> {
    /// the index of the strategy in the list of strategies from the configuration file
    index: usize,
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,
    strategy_thread: thread::JoinHandle<()>,
}

struct TheStaticNeighbourhoodsAdaptor<T: StackType, P: ProcessAdaptor<T>> {
    _phantom: PhantomData<T>,
    strategy_index: usize,
    process_adaptor: P,
}

impl<T: StackType, P: ProcessAdaptor<T> + 'static> StrategyAdaptor<T>
    for TheStaticNeighbourhoodsAdaptor<T, P>
{
    #[inline]
    fn send(&self, msg: FromStrategy<T>) -> bool {
        self.process_adaptor.send(FromProcess::FromStrategy(msg))
    }

    #[inline]
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.process_adaptor.key_state(i)
    }

    #[inline]
    fn tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.process_adaptor.tuning(i)
    }

    #[inline]
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>> {
        self.process_adaptor.tuning_reference()
    }
}

impl<T: StackType, P: ProcessAdaptor<T> + 'static> StaticNeighbourhoodsAdaptor<T>
    for TheStaticNeighbourhoodsAdaptor<T, P>
{
    fn config(&self) -> impl DerefMut<Target = StaticNeighbourhoodsConfig<T>> {
        self.process_adaptor
            .config()
            .map(
                |c: &mut Vec<StrategyConfig<T>>| match &mut c[self.strategy_index] {
                    StrategyConfig::StaticNeighbourhoods { config, .. } => config,
                    _ => panic!("TheStaticNeighbourhoodsAdaptor::config: incorrect config type"),
                },
            )
    }
}

struct TheTwoStepAdaptor<T: StackType, P: ProcessAdaptor<T>> {
    process_adaptor: P,
    strategy_index: usize,
    harmony: Arc<RwLock<Harmony<T>>>,
}

impl<T: StackType, P: ProcessAdaptor<T>> MelodyStrategyAdaptor<T> for TheTwoStepAdaptor<T, P> {
    #[inline]
    fn send(&self, msg: FromStrategy<T>) -> bool {
        self.process_adaptor.send(FromProcess::FromStrategy(msg))
    }

    #[inline]
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.process_adaptor.key_state(i)
    }

    #[inline]
    fn tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.process_adaptor.tuning(i)
    }

    #[inline]
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>> {
        self.process_adaptor.tuning_reference()
    }

    #[inline]
    fn harmony(&self) -> impl Deref<Target = Harmony<T>> {
        self.harmony.read()
    }
}

impl<T: StackType, P: ProcessAdaptor<T>> HarmonyStrategyAdaptor<T> for TheTwoStepAdaptor<T, P> {
    #[inline]
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.process_adaptor.key_state(i)
    }

    #[inline]
    fn harmony(&self) -> impl DerefMut<Target = Harmony<T>> {
        self.harmony.write()
    }
}

impl<T: StackType, P: ProcessAdaptor<T>> StrategyAdaptor<T> for TheTwoStepAdaptor<T, P> {
    #[inline]
    fn send(&self, msg: FromStrategy<T>) -> bool {
        self.process_adaptor.send(FromProcess::FromStrategy(msg))
    }

    #[inline]
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.process_adaptor.key_state(i)
    }

    #[inline]
    fn tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.process_adaptor.tuning(i)
    }

    #[inline]
    fn tuning_reference(&self) -> impl Deref<Target=Reference<T>> {
        self.process_adaptor.tuning_reference()
    }
}

impl<T: StackType, P: ProcessAdaptor<T>> StaticNeighbourhoodsAsMelodyAdaptor<T>
    for TheTwoStepAdaptor<T, P>
{
    fn config(&self) -> impl DerefMut<Target = StaticNeighbourhoodsAsMelodyConfig<T>> {
        self.process_adaptor
            .config()
            .map(
                |c: &mut Vec<StrategyConfig<T>>| match &mut c[self.strategy_index] {
                    StrategyConfig::TwoStep { melody: MelodyStrategyConfig::StaticNeighbourhoods(config), .. } => config,
                    _ => panic!("TheTwoStepAdaptor::config: incorrect melody config type for static neighbourhoods"),
                },
            )
    }
}

impl<T: StackType, P: ProcessAdaptor<T>> ChordListAdaptor<T> for TheTwoStepAdaptor<T, P> {
    fn config(&self) -> impl DerefMut<Target = ChordListConfig<T>> {
        self.process_adaptor
            .config()
            .map(
                |c: &mut Vec<StrategyConfig<T>>| match &mut c[self.strategy_index] {
                    StrategyConfig::TwoStep {
                        harmony: HarmonyStrategyConfig::ChordList(config),
                        ..
                    } => config,
                    _ => panic!(
                        "TheTwoStepAdaptor::config: incorrect harmony config type for chord list"
                    ),
                },
            )
    }
}

impl<T: StackType, P: ProcessAdaptor<T>>
    TwoStepStrategyAdaptor<T, ChordList<T>, Self, StaticNeighbourhoodsAsMelody<T>, Self>
    for TheTwoStepAdaptor<T, P>
{
    fn as_melody_adaptor(&self) -> &Self {
        self
    }

    fn as_harmony_adaptor(&self) -> &Self {
        self
    }
}

impl<T: StackType + Send + Sync> RunningStrategy<T> {
    fn start<S, A>(time: Instant, index: usize, config: S::Config, adaptor: A) -> Self
    where
        S: Strategy<T, A>,
        S::Config: Send + 'static,
        A: StrategyAdaptor<T> + Send + 'static,
    {
        let (to_strategy_tx, to_strategy_rx) = mpsc::channel();

        let strategy_thread = thread::spawn(move || {
            let mut strategy = S::new(config);
            strategy.start(time, &adaptor);
            strategy.receive_solve_loop(to_strategy_rx, &adaptor);
        });

        Self {
            index,
            strategy_thread,
            to_strategy_tx,
        }
    }

    /// returns the index of the strategy that was stopped
    fn stop(self, time: Instant) -> usize {
        let _ = self.to_strategy_tx.send(ToStrategy::Stop { time });

        self.strategy_thread
            .join()
            .unwrap_or_else(|_| panic!("Could not join running strategy thread"));

        self.index
    }
}

pub struct ProcessFromStrategy<T: StackType, A: ProcessAdaptor<T>> {
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],

    current_strategy: Option<RunningStrategy<T>>,

    adaptor: A,
}

impl<T, P> ProcessFromStrategy<T, P>
where
    T: StackType + Send + Sync,
    P: ProcessAdaptor<T> + Send + 'static,
{
    pub fn new(adaptor: P) -> Self {
        if adaptor.config().len() <= 0 {
            panic!("Cannot start process from empty list of strategies");
        }

        Self {
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            current_strategy: None {},
            adaptor,
        }
    }

    fn send_to_strategy(&self, msg: ToStrategy<T>) {
        if let Some(RunningStrategy { to_strategy_tx, .. }) = &self.current_strategy {
            let _ = to_strategy_tx.send(msg);
        }
    }

    fn current_strategy_index(&self) -> Option<usize> {
        if let Some(RunningStrategy { index, .. }) = &self.current_strategy {
            Some(*index)
        } else {
            None {}
        }
    }

    fn handle_midi(&mut self, time: Instant, msg: MidiMsg) {
        let untouched_midi = || FromProcess::OutgoingMidi {
            bytes: msg.to_midi(),
            time,
        };

        match msg {
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOn { note, velocity },
            } => {
                if velocity != 0 {
                    self.handle_note_on(time, note, channel, velocity);
                } else {
                    self.handle_note_off(time, note, channel, 0);
                }
            }
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOff { note, velocity },
            } => self.handle_note_off(time, note, channel, velocity),
            MidiMsg::ChannelVoice {
                channel,
                msg: ControlChange {
                    control: Hold(value),
                },
            } => self.handle_pedal_hold(time, channel, value),

            // MidiMsg::ChannelVoice {
            //     channel,
            //     msg:
            //         ControlChange {
            //             control: Sostenuto(value),
            //         },
            // } => {
            //     if let Some(csi) = self.current_strategy_index() {
            //         let (_, ref bindings) = self.strategies[csi];
            //         let was_down = self.sostenuto_hold.iter().any(|b| *b);
            //         self.sostenuto_hold[channel as usize] = value > 0;
            //         let is_down = self.sostenuto_hold.iter().any(|b| *b);
            //         let action = match (was_down, is_down) {
            //             (false, true) => bindings.get(&MidiBindable::SostenutoPedalDown),
            //             (true, false) => bindings.get(&MidiBindable::SostenutoPedalUp),
            //             _ => None {},
            //         };
            //         if let Some(&action) = action {
            //             let _ = self.send_to_strategy(ToStrategy::Action { action, time });
            //         } else {
            //             self.adaptor.send(untouched_midi());
            //         }
            //     }
            // }

            // MidiMsg::ChannelVoice {
            //     channel,
            //     msg:
            //         ControlChange {
            //             control: SoftPedal(value),
            //         },
            // } => {
            //     if let Some(csi) = self.current_strategy_index() {
            //         let (_, ref bindings) = self.strategies[csi];
            //         let was_down = self.soft_hold.iter().any(|b| *b);
            //         self.soft_hold[channel as usize] = value > 0;
            //         let is_down = self.soft_hold.iter().any(|b| *b);
            //         let action = match (was_down, is_down) {
            //             (false, true) => bindings.get(&MidiBindable::SoftPedalDown),
            //             (true, false) => bindings.get(&MidiBindable::SoftPedalUp),
            //             _ => None {},
            //         };
            //         if let Some(&action) = action {
            //             let _ = self.send_to_strategy(ToStrategy::Action { action, time });
            //         } else {
            //             self.adaptor.send(untouched_midi());
            //         }
            //     }
            // }
            MidiMsg::ChannelVoice {
                channel,
                msg: ProgramChange { program },
            } => {
                let _ = self.adaptor.send(FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                });
            }

            _ => {
                let _ = self.adaptor.send(untouched_midi());
            }
        }
    }

    fn handle_note_on(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if self.adaptor.key_state(note as usize).note_on(channel, time) {
                let _ = self.send_to_strategy(ToStrategy::NoteOn { note, time });
            }
            let _ = self.adaptor.send(FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_note_off(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if self.adaptor.key_state(note as usize).note_off(
                channel,
                self.pedal_hold[channel as usize],
                time,
            ) {
                let _ = self.send_to_strategy(ToStrategy::NoteOff { note, time });
            }
            let _ = self.adaptor.send(FromProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_pedal_hold(&mut self, time: Instant, channel: Channel, value: u8) {
        if self.current_strategy_index().is_some() {
            if value > 0 {
                self.pedal_hold[channel as usize] = true;
            } else {
                self.pedal_hold[channel as usize] = false;
                for i in 0..128 {
                    let changed = self.adaptor.key_state(i).pedal_off(channel, time);
                    if changed {
                        let _ = self.send_to_strategy(ToStrategy::NoteOff {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
            let _ = self.adaptor.send(FromProcess::PedalHold {
                channel,
                value,
                time,
            });
        }
    }

    /// Returns the index of the previously running strategy, if any.
    fn stop(&mut self, time: Instant) -> Option<usize> {
        if let Some(rs) = self.current_strategy.take() {
            Some(rs.stop(time))
        } else {
            None {}
        }
    }

    /// Will do a restart if there's already a strategy running
    fn start(&mut self, time: Instant, index: usize) {
        if self.current_strategy.is_some() {
            self.stop(time);
        }

        match &self.adaptor.config()[index] {
            StrategyConfig::StaticNeighbourhoods { config, .. } => {
                self.current_strategy = Some(RunningStrategy::start::<StaticNeighbourhoods<T>, _>(
                    time,
                    index,
                    config.clone(),
                    TheStaticNeighbourhoodsAdaptor {
                        _phantom: PhantomData,
                        strategy_index: index,
                        process_adaptor: self.adaptor.clone(),
                    },
                ))
            }
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::ChordList(harmony_config),
                melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                ..
            } => {
                self.current_strategy = Some(RunningStrategy::start::<
                    TwoStep<T, ChordList<T>, _, StaticNeighbourhoodsAsMelody<T>, _>,
                    _,
                >(
                    time,
                    index,
                    (harmony_config.clone(), melody_config.clone()),
                    TheTwoStepAdaptor {
                        strategy_index: index,
                        process_adaptor: self.adaptor.clone(),
                        harmony: Arc::new(RwLock::new(Harmony::new_dummy())),
                    },
                ))
            }
        }

        self.adaptor
            .send(FromProcess::CurrentStrategyIndex(Some(index)));
    }

    /// Will start strategy 0 if there's no running strategy at the moment.
    fn restart(&mut self, time: Instant) {
        let index = self.stop(time).unwrap_or(0);
        self.start(time, index);
    }
}

impl<T, A> ReceiveMsg<ToProcess<T>> for ProcessFromStrategy<T, A>
where
    T: StackType + fmt::Debug + Send + Sync,
    A: ProcessAdaptor<T> + Send + 'static,
{
    fn receive_msg(&mut self, msg: ToProcess<T>) {
        match msg {
            ToProcess::Stop { time } => {
                let _ = self.stop(time);
            }
            ToProcess::Reset { time } => self.restart(time),
            ToProcess::Start { time } => self.restart(time),
            ToProcess::IncomingMidi { time, bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg), // TODO: multi-part messages?
                Err(e) => {
                    let _ = self.adaptor.send(FromProcess::MidiParseErr(e.to_string()));
                }
            },
            ToProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_on(time, note, channel, velocity),
            ToProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_off(time, note, channel, velocity),
            ToProcess::PedalHold {
                channel,
                value,
                time,
            } => self.handle_pedal_hold(time, channel, value),
            ToProcess::ToStrategy(msg) => {
                if self.current_strategy_index().is_some() {
                    let _ = self.send_to_strategy(msg);
                }
            }
            ToProcess::RestartFromConfig { time } => {
                self.restart(time);
            }
            ToProcess::BindAction { action, bindable } => {
                todo!()
                // if let Some(csi) = self.current_strategy_index() {
                //     let (_, bindings) = &mut self.strategies[csi];
                //     if let Some(action) = action {
                //         bindings.insert(bindable, action);
                //     } else {
                //         bindings.remove(&bindable);
                //     }
                // }
            }
        }
    }
}
