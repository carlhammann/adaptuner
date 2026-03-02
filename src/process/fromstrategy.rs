use std::{fmt, sync::mpsc, thread, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg::*, ControlChange::Hold, MidiMsg};

use crate::{
    bindable::{Bindings, MidiBindable},
    config::{
        FromConfigAndState, HarmonyStrategyConfig, IsStrategyConfig, MelodyStrategyConfig,
        ProcessConfig, StrategyConfig,
    },
    interval::stacktype::r#trait::StackType,
    msg::{FromProcess, FromStrategy, ReceiveMsg, ToProcess, ToStrategy},
    process::r#trait::{ConcreteProcessAdaptor, ProcessAdaptor},
    strategy::r#trait::{ConcreteStrategyAdaptor, Strategy},
};

struct RunningStrategy<T: StackType> {
    /// The index in the list of strategies
    index: usize,
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,
    strategy_thread: thread::JoinHandle<()>,
}

impl<T: StackType + Send + Sync> RunningStrategy<T> {
    fn start(
        time: Instant,
        index: usize,
        config: impl IsStrategyConfig<T> + Send + 'static,
        adaptor: ConcreteStrategyAdaptor<T>,
    ) -> Self {
        let (to_strategy_tx, to_strategy_rx) = mpsc::channel();

        let strategy_thread = thread::spawn(move || {
            let mut strategy = config.realize();

            strategy.start(time, &adaptor);

            strategy.receive_solve_loop(to_strategy_rx, &adaptor)
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

pub struct ProcessFromStrategy<T: StackType> {
    strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],

    _message_forward_thread: thread::JoinHandle<mpsc::Receiver<FromStrategy<T>>>,

    current_strategy: Option<RunningStrategy<T>>,
    from_strategy_tx: mpsc::Sender<FromStrategy<T>>,

    adaptor: ConcreteProcessAdaptor<T>,
}

impl<T: StackType + Send + Sync> ProcessFromStrategy<T> {
    pub fn new(
        strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
        adaptor: ConcreteProcessAdaptor<T>,
    ) -> Self {
        if strategies.len() <= 0 {
            panic!("Cannot start process from empty list of strategies");
        }

        let (from_strategy_tx, from_strategy_rx) = mpsc::channel();

        let forward_clone = adaptor.forward.clone();
        let message_forward_thread = thread::spawn(move || {
            loop {
                match from_strategy_rx.recv() {
                    Ok(msg) => {
                        let _ = forward_clone.send(FromProcess::FromStrategy(msg));
                    }
                    Err(_) => break,
                }
            }
            from_strategy_rx
        });

        Self {
            strategies,
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            current_strategy: None {},
            _message_forward_thread: message_forward_thread,
            from_strategy_tx,
            adaptor,
        }
    }
}

impl<T: StackType + Send + Sync> ProcessFromStrategy<T> {
    fn send_msg(&self, msg: FromProcess<T>) -> bool {
        self.adaptor.send(msg)
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
            } => self.handle_note_on(time, note, channel, velocity),
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
            //             self.send_msg(untouched_midi());
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
            //             self.send_msg(untouched_midi());
            //         }
            //     }
            // }
            MidiMsg::ChannelVoice {
                channel,
                msg: ProgramChange { program },
            } => {
                let _ = self.send_msg(FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                });
            }

            _ => {
                let _ = self.send_msg(untouched_midi());
            }
        }
    }

    fn handle_note_on(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if self
                .adaptor
                .write_key_state(note as usize)
                .note_on(channel, time)
            {
                let _ = self.send_to_strategy(ToStrategy::NoteOn { note, time });
            }
            let _ = self.send_msg(FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_note_off(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if self.adaptor.write_key_state(note as usize).note_off(
                channel,
                self.pedal_hold[channel as usize],
                time,
            ) {
                let _ = self.send_to_strategy(ToStrategy::NoteOff { note, time });
            }
            let _ = self.send_msg(FromProcess::NoteOff {
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
                    let changed = self.adaptor.write_key_state(i).pedal_off(channel, time);
                    if changed {
                        let _ = self.send_to_strategy(ToStrategy::NoteOff {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
            let _ = self.send_msg(FromProcess::PedalHold {
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
        self.current_strategy = Some(match &self.strategies[index].0 {
            StrategyConfig::TwoStep(
                HarmonyStrategyConfig::ChordList(harmony),
                MelodyStrategyConfig::Neighbourhoods(melody),
            ) => RunningStrategy::start(
                time,
                index,
                (harmony.clone(), melody.clone()),
                ConcreteStrategyAdaptor {
                    forward: self.from_strategy_tx.clone(),
                    key_states: self.adaptor.key_states.clone().into_reader(),
                    tunings: self.adaptor.tunings.clone(),
                },
            ),
            StrategyConfig::StaticTuning(conf) => RunningStrategy::start(
                time,
                index,
                conf.clone(),
                ConcreteStrategyAdaptor {
                    forward: self.from_strategy_tx.clone(),
                    key_states: self.adaptor.key_states.clone().into_reader(),
                    tunings: self.adaptor.tunings.clone(),
                },
            ),
        });
        self.adaptor
            .send(FromProcess::CurrentStrategyIndex(Some(index)));
    }

    /// Will start strategy 0 if there's no running strategy at the moment.
    fn restart(&mut self, time: Instant) {
        let index = self.stop(time).unwrap_or(0);
        self.start(time, index);
    }
}

impl<T: StackType + fmt::Debug + Send + Sync> ReceiveMsg<ToProcess<T>> for ProcessFromStrategy<T> {
    fn receive_msg(&mut self, msg: ToProcess<T>) {
        match msg {
            ToProcess::Stop => {}
            ToProcess::Reset { time } => self.restart(time),
            ToProcess::Start { time } => self.restart(time),
            ToProcess::IncomingMidi { time, bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg), // TODO: multi-part messages?
                Err(e) => {
                    let _ = self.send_msg(FromProcess::MidiParseErr(e.to_string()));
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
            ToProcess::StrategyListAction { action, time } => {
                let mut index = self.stop(time);
                action.apply_to(
                    |(strat, bind)| (strat.clone(), bind.clone()),
                    &mut self.strategies,
                    &mut index,
                );
                if let Some(index) = index {
                    self.start(time, index);
                }
            }
            ToProcess::BindAction { action, bindable } => {
                if let Some(csi) = self.current_strategy_index() {
                    let (_, bindings) = &mut self.strategies[csi];
                    if let Some(action) = action {
                        bindings.insert(bindable, action);
                    } else {
                        bindings.remove(&bindable);
                    }
                }
            }
        }
    }
}

impl<T: StackType + Send + Sync> FromConfigAndState<ProcessConfig<T>, ConcreteProcessAdaptor<T>>
    for ProcessFromStrategy<T>
{
    fn initialise(config: ProcessConfig<T>, adaptor: ConcreteProcessAdaptor<T>) -> Self {
        let ProcessConfig { strategies } = config;
        Self::new(strategies, adaptor)
    }
}
