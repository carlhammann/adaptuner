use std::{fmt, sync::mpsc, sync::Arc, thread, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    adaptors::{take_replace, ConcreteLocks},
    bindable::{BindableEvent, BindableProcessAction},
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    interval::stacktype::r#trait::StackType,
    msg::{FromProcess, ReceiveMsg, ToProcess, ToStrategy},
    process::r#trait::ProcessAdaptor,
    strategy::{
        harmony::chordlist::ChordList,
        melody::neighbourhoods::StaticNeighbourhoodsAsMelody,
        r#trait::{Strategy, StrategyAdaptor},
        staticneighbourhoods::StaticNeighbourhoods,
        twostep::TwoStep,
    },
    util::ordered_locks::Zero,
};

struct RunningStrategy<T: StackType> {
    /// the index of the strategy in the list of strategies from the configuration file
    index: usize,
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,
    strategy_thread: thread::JoinHandle<()>,
}

impl<T: StackType + Send + Sync> RunningStrategy<T> {
    fn start<S>(
        time: Instant,
        index: usize,
        config: S::Config,
        adaptor_inner: Arc<ConcreteLocks<T>>,
    ) -> Self
    where
        S: Strategy<T>,
        S::Config: Send + 'static,
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

pub struct ProcessFromStrategy<T: StackType> {
    current_strategy: Option<RunningStrategy<T>>,

    /// The `Option` is only there in order to use Option::take, which allows the "consuming" style
    /// of the [OrderedLocks] functions. This is facilitated by [ProcessFromStrategy::on_adaptor]
    adaptor: Option<ProcessAdaptor<T, Zero>>,
}

impl<T> ProcessFromStrategy<T>
where
    T: StackType + Send + Sync,
{
    pub fn new(adaptor: ProcessAdaptor<T, Zero>) -> Self {
        Self {
            current_strategy: None {},
            adaptor: Some(adaptor),
        }
    }

    #[inline]
    fn send(&self, msg: FromProcess<T>) {
        self.adaptor.as_ref().unwrap().send(msg);
    }

    #[inline]
    fn send_to_strategy(&self, msg: ToStrategy<T>) {
        if let Some(RunningStrategy { to_strategy_tx, .. }) = &self.current_strategy {
            let _ = to_strategy_tx.send(msg);
        }
    }

    #[inline]
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
                let (was_down, is_down) = take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.sostenuto_hold_mut(|bs, _| {
                        let was = bs.iter().any(|b| *b);
                        bs[channel as usize] = value > 0;
                        let is = bs.iter().any(|b| *b);
                        (was, is)
                    })
                });
                let action = match (was_down, is_down) {
                    (false, true) => take_replace(&mut self.adaptor, |adaptor| {
                        adaptor.active_strategy(|strat, _| {
                            strat
                                .bindings()
                                .get(&BindableEvent::SostenutoPedalDown)
                                .map(|x| *x)
                        })
                    }),
                    (true, false) => take_replace(&mut self.adaptor, |adaptor| {
                        adaptor.active_strategy(|strat, _| {
                            strat
                                .bindings()
                                .get(&BindableEvent::SostenutoPedalUp)
                                .map(|x| *x)
                        })
                    }),
                    _ => None {},
                };

                match action {
                    Some(BindableProcessAction::Reset) => self.restart(time),
                    Some(BindableProcessAction::ToStrategy(action)) => {
                        self.send_to_strategy(ToStrategy::BoundAction { action, time })
                    }
                    None {} => self.send(untouched_midi()),
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::SoftPedal(value),
                    },
            } => {
                let (was_down, is_down) = take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.soft_hold_mut(|bs, _| {
                        let was = bs.iter().any(|b| *b);
                        bs[channel as usize] = value > 0;
                        let is = bs.iter().any(|b| *b);
                        (was, is)
                    })
                });
                let action = match (was_down, is_down) {
                    (false, true) => take_replace(&mut self.adaptor, |adaptor| {
                        adaptor.active_strategy(|strat, _| {
                            strat
                                .bindings()
                                .get(&BindableEvent::SoftPedalDown)
                                .map(|x| *x)
                        })
                    }),
                    (true, false) => take_replace(&mut self.adaptor, |adaptor| {
                        adaptor.active_strategy(|strat, _| {
                            strat
                                .bindings()
                                .get(&BindableEvent::SoftPedalUp)
                                .map(|x| *x)
                        })
                    }),
                    _ => None {},
                };

                match action {
                    Some(BindableProcessAction::Reset) => self.restart(time),
                    Some(BindableProcessAction::ToStrategy(action)) => {
                        self.send_to_strategy(ToStrategy::BoundAction { action, time })
                    }
                    None {} => self.send(untouched_midi()),
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::ProgramChange { program },
            } => {
                let _ = self.send(FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                });
            }

            _ => {
                let _ = self.send(untouched_midi());
            }
        }
    }

    fn handle_note_on(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if take_replace(&mut self.adaptor, |adaptor| {
                adaptor.key_state_mut(note as usize, |k, _| k.note_on(channel, time))
            }) {
                let _ = self.send_to_strategy(ToStrategy::NoteOn { note, time });
            }
            let _ = self.send(FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_note_off(&mut self, time: Instant, note: u8, channel: Channel, velocity: u8) {
        if self.current_strategy_index().is_some() {
            if take_replace(&mut self.adaptor, |mut adaptor| {
                let pedal_hold;
                (pedal_hold, adaptor) = adaptor.pedal_hold(|bs, _| bs[channel as usize]);
                adaptor.key_state_mut(note as usize, |k, _| k.note_off(channel, pedal_hold, time))
            }) {
                let _ = self.send_to_strategy(ToStrategy::NoteOff { note, time });
            }
            let _ = self.send(FromProcess::NoteOff {
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
                take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.pedal_hold_mut(|pedal_hold, _| pedal_hold[channel as usize] = true)
                });
            } else {
                take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.pedal_hold_mut(|pedal_hold, _| pedal_hold[channel as usize] = false)
                });
                for i in 0..128 {
                    if take_replace(&mut self.adaptor, |adaptor| {
                        adaptor.key_state_mut(i, |k, _| k.pedal_off(channel, time))
                    }) {
                        let _ = self.send_to_strategy(ToStrategy::NoteOff {
                            note: i as u8,
                            time,
                        });
                    }
                }
            }
            let _ = self.send(FromProcess::PedalHold {
                channel,
                value,
                time,
            });
            if value > 0 {
                self.pedal_hold[channel as usize] = true;
            } else {
                self.pedal_hold[channel as usize] = false;
                let mut any_off = false;
                for i in 0..128 {
                    any_off |= self.key_states[i].pedal_off(channel, time);
                }
                if any_off {
                    let _ = self.strategies[csi].0.note_off(
                        &self.key_states,
                        &mut self.tunings,
                        time,
                        &mut self.queue,
                    );
                    self.queue.drain(..).for_each(|msg| {
                        let _ = forward.send(FromProcess::FromStrategy(msg));
                    });
                }
            }
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

        take_replace(&mut self.adaptor, |adaptor| {
            adaptor.active_strategy(|strat, adaptor| match strat {
                StrategyConfig::StaticNeighbourhoods { config, .. } => {
                    self.current_strategy = Some(RunningStrategy::start::<StaticNeighbourhoods<T>>(
                        time,
                        index,
                        config.clone(),
                        unsafe { adaptor.inner_arc() },
                    ))
                }
                StrategyConfig::TwoStep {
                    harmony: HarmonyStrategyConfig::ChordList(harmony_config),
                    melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                    ..
                } => {
                    self.current_strategy = Some(RunningStrategy::start::<
                        TwoStep<T, ChordList<T>, StaticNeighbourhoodsAsMelody<T>>,
                    >(
                        time,
                        index,
                        (harmony_config.clone(), melody_config.clone()),
                        unsafe { adaptor.inner_arc() },
                    ))
                }
            })
        });

        self.send(FromProcess::StartedStrategy {
            index: Some(index),
            time,
        })
    }

    /// Will start strategy 0 if there's no running strategy at the moment.
    fn restart(&mut self, time: Instant) {
        let index = self.stop(time).unwrap_or(0);
        self.start(time, index);
    }
}

impl<T> ReceiveMsg<ToProcess<T>> for ProcessFromStrategy<T>
where
    T: StackType + fmt::Debug + Send + Sync,
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
                    let _ = self.send(FromProcess::MidiParseErr(e.to_string()));
                }
            },
            ToProcess::ToStrategy(msg) => {
                let _ = self.to_strategy_tx.send(msg);
            }
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
            ToProcess::BoundAction { action, time } => match action {
                BindableProcessAction::Reset => self.restart(time),
                BindableProcessAction::ToStrategy(action) => {
                    self.send_to_strategy(ToStrategy::BoundAction { action, time })
                }
            },
        }
    }
}
