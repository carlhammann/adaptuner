use std::{fmt, sync::mpsc, sync::Arc, thread, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    bindable::{BindableEvent, BindableProcessAction},
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    interval::stacktype::r#trait::StackType,
    msg::{FromProcess, ReceiveMsg, ToProcess, ToStrategy},
    process::r#trait::{ProcessAdaptor, ProcessTag},
    strategy::{
        harmony::chordlist::ChordList,
        melody::neighbourhoods::StaticNeighbourhoodsAsMelody,
        r#trait::{Strategy, StrategyAdaptor},
        staticneighbourhoods::StaticNeighbourhoods,
        twostep::TwoStep,
    },
    util::ordered_locks::OrderedLocks,
};

struct RunningStrategy<T: StackType> {
    /// the index of the strategy in the list of strategies from the configuration file
    index: usize,
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,
    strategy_thread: thread::JoinHandle<()>,
}

impl<T: StackType + Send + Sync> RunningStrategy<T> {
    fn start<S, P>(time: Instant, index: usize, config: S::Config, adaptor_inner: Arc<P>) -> Self
    where
        S: Strategy<T>,
        S::Config: Send + 'static,
        P: ProcessAdaptor<StackType = T> + 'static,
    {
        let (to_strategy_tx, to_strategy_rx) = mpsc::channel();

        let strategy_thread = thread::spawn(move || {
            let mut strategy = S::new(config);
            let mut adaptor = unsafe { StrategyAdaptor::new_zero(adaptor_inner) };
            let needs_steps_at_first_iteration;
            (needs_steps_at_first_iteration, adaptor) = strategy.start(time, adaptor);
            strategy.receive_solve_loop(needs_steps_at_first_iteration, to_strategy_rx, adaptor);
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

pub struct ProcessFromStrategy<T: StackType, A: ProcessAdaptor<StackType = T>> {
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],

    current_strategy: Option<RunningStrategy<T>>,

    adaptor: Arc<A>,
}

impl<T, P> ProcessFromStrategy<T, P>
where
    T: StackType + Send + Sync,
    P: ProcessAdaptor<StackType = T> + Send + 'static,
{
    pub fn new(adaptor: P) -> Self {
        Self {
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            current_strategy: None {},
            adaptor: Arc::new(adaptor),
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
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            } => {
                if velocity != 0 {
                    self.handle_note_on(time, note, channel, velocity);
                } else {
                    self.handle_note_off(time, note, channel, 0);
                }
            }
            MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::NoteOff { note, velocity },
            } => self.handle_note_off(time, note, channel, velocity),
            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Hold(value),
                    },
            } => self.handle_pedal_hold(time, channel, value),

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Sostenuto(value),
                    },
            } => {
                let was_down = self.sostenuto_hold.iter().any(|b| *b);
                self.sostenuto_hold[channel as usize] = value > 0;
                let is_down = self.sostenuto_hold.iter().any(|b| *b);
                let action = match (was_down, is_down) {
                    (false, true) => {
                        let locks = unsafe {
                            OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone())
                        };
                        locks
                            .active_strategy(|strat, _| {
                                strat
                                    .bindings()
                                    .get(&BindableEvent::SostenutoPedalDown)
                                    .map(|x| *x)
                            })
                            .0
                    }
                    (true, false) => {
                        let locks = unsafe {
                            OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone())
                        };
                        locks
                            .active_strategy(|strat, _| {
                                strat
                                    .bindings()
                                    .get(&BindableEvent::SostenutoPedalUp)
                                    .map(|x| *x)
                            })
                            .0
                    }
                    _ => None {},
                };

                match action {
                    Some(BindableProcessAction::Reset) => self.restart(time),
                    Some(BindableProcessAction::ToStrategy(action)) => {
                        self.send_to_strategy(ToStrategy::BoundAction { action, time })
                    }
                    None {} => self.adaptor.send(untouched_midi()),
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::SoftPedal(value),
                    },
            } => {
                let was_down = self.soft_hold.iter().any(|b| *b);
                self.soft_hold[channel as usize] = value > 0;
                let is_down = self.soft_hold.iter().any(|b| *b);
                let action = match (was_down, is_down) {
                    (false, true) => {
                        let locks = unsafe {
                            OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone())
                        };
                        locks
                            .active_strategy(|strat, _| {
                                strat
                                    .bindings()
                                    .get(&BindableEvent::SoftPedalDown)
                                    .map(|x| *x)
                            })
                            .0
                    }
                    (true, false) => {
                        let locks = unsafe {
                            OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone())
                        };
                        locks
                            .active_strategy(|strat, _| {
                                strat
                                    .bindings()
                                    .get(&BindableEvent::SoftPedalUp)
                                    .map(|x| *x)
                            })
                            .0
                    }
                    _ => None {},
                };

                match action {
                    Some(BindableProcessAction::Reset) => self.restart(time),
                    Some(BindableProcessAction::ToStrategy(action)) => {
                        self.send_to_strategy(ToStrategy::BoundAction { action, time })
                    }
                    None {} => self.adaptor.send(untouched_midi()),
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::ProgramChange { program },
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
            let locks = unsafe { OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone()) };
            if locks
                .key_state_mut(note as usize, |k, _| k.note_on(channel, time))
                .0
            {
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
            let locks = unsafe { OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone()) };
            if locks
                .key_state_mut(note as usize, |k, _| {
                    k.note_off(channel, self.pedal_hold[channel as usize], time)
                })
                .0
            {
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
                let mut locks =
                    unsafe { OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone()) };
                for i in 0..128 {
                    let changed;
                    (changed, locks) = locks.key_state_mut(i, |k, _| k.pedal_off(channel, time));
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

        {
            let locks = unsafe { OrderedLocks::<ProcessTag, _, _>::new_zero(self.adaptor.clone()) };

            locks.active_strategy(|strat, _| match strat {
                StrategyConfig::StaticNeighbourhoods { config, .. } => {
                    self.current_strategy =
                        Some(RunningStrategy::start::<StaticNeighbourhoods<T>, _>(
                            time,
                            index,
                            config.clone(),
                            self.adaptor.clone(),
                        ))
                }
                StrategyConfig::TwoStep {
                    harmony: HarmonyStrategyConfig::ChordList(harmony_config),
                    melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                    ..
                } => {
                    self.current_strategy = Some(RunningStrategy::start::<
                        TwoStep<T, ChordList<T>, StaticNeighbourhoodsAsMelody<T>>,
                        _,
                    >(
                        time,
                        index,
                        (harmony_config.clone(), melody_config.clone()),
                        self.adaptor.clone(),
                    ))
                }
            });
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
    A: ProcessAdaptor<StackType = T> + Send + 'static,
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
            ToProcess::BoundAction { action, time } => match action {
                BindableProcessAction::Reset => self.restart(time),
                BindableProcessAction::ToStrategy(action) => {
                    self.send_to_strategy(ToStrategy::BoundAction { action, time })
                }
            },
        }
    }
}
