use std::{fmt, sync::Arc};

use crate::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
use std::{sync::mpsc, thread};

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::interval::stacktype::r#trait::FiveLimitIntervalBasis;

use crate::{
    adaptors::{take_replace, ConcreteLocks},
    bindable::{BindableEvent, BindableProcessAction},
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    interval::{
        fundamental::{HasFundamental, HasOvertone},
        stacktype::r#trait::StackType,
    },
    msg::{FromProcess, ReceiveMsg, ToProcess, ToStrategy},
    process::r#trait::ProcessAdaptor,
    strategy::{
        harmony::{chordlist::ChordList, list::ListOfHarmonyStrategies, springs::HarmonySprings},
        melody::neighbourhoods::StaticNeighbourhoodsAsMelody,
        r#trait::{Strategy, StrategyAdaptor},
        staticneighbourhoods::StaticNeighbourhoods,
        twostep::TwoStep,
    },
    util::ordered_locks::Zero,
};

enum StrategyRuntime<T: StackType>
where
    T: Send + Sync + FiveLimitIntervalBasis,
{
    StaticNeighbourhoods(StaticNeighbourhoods<T>),
    TwoStepChordList(TwoStep<T, ChordList<T>, StaticNeighbourhoodsAsMelody<T>>),
    TwoStepSprings(TwoStep<T, HarmonySprings<T>, StaticNeighbourhoodsAsMelody<T>>),
    TwoStepList(TwoStep<T, ListOfHarmonyStrategies<T>, StaticNeighbourhoodsAsMelody<T>>),
}

impl<T> StrategyRuntime<T>
where
    T: StackType + Send + Sync + FiveLimitIntervalBasis,
{
    fn from_config(config: &StrategyConfig<T>) -> Self {
        match config {
            StrategyConfig::StaticNeighbourhoods { config, .. } => {
                Self::StaticNeighbourhoods(StaticNeighbourhoods::new(config.clone()))
            }

            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::ChordList(harmony_config),
                melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                ..
            } => Self::TwoStepChordList(TwoStep::new((
                harmony_config.clone(),
                melody_config.clone(),
            ))),

            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::Springs(harmony_config),
                melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                ..
            } => Self::TwoStepSprings(TwoStep::new((
                harmony_config.clone(),
                melody_config.clone(),
            ))),

            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::List(harmony_configs),
                melody: MelodyStrategyConfig::StaticNeighbourhoods(melody_config),
                ..
            } => Self::TwoStepList(TwoStep::new((
                harmony_configs.clone(),
                melody_config.clone(),
            ))),
        }
    }

    fn start(&mut self, time: Instant, locks: Arc<ConcreteLocks<T>>) {
        match self {
            Self::StaticNeighbourhoods(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.start(time, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepChordList(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.start(time, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepSprings(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.start(time, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepList(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.start(time, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }
        }
    }

    fn process_message(&mut self, msg: ToStrategy<T>, locks: Arc<ConcreteLocks<T>>) {
        match self {
            Self::StaticNeighbourhoods(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.process_message(msg, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepChordList(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.process_message(msg, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepSprings(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.process_message(msg, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }

            Self::TwoStepList(strategy) => {
                let adaptor = unsafe { StrategyAdaptor::new_zero(locks) };
                let (mut needs_steps, mut adaptor) = strategy.process_message(msg, adaptor);

                while needs_steps {
                    (needs_steps, adaptor) = strategy.step(adaptor);
                }
            }
        }
    }
}

struct RunningStrategy<T>
where
    T: StackType + Send + Sync + FiveLimitIntervalBasis,
{
    /// the index of the strategy in the list of strategies from the configuration file
    index: usize,
    #[cfg(not(target_arch = "wasm32"))]
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,

    #[cfg(not(target_arch = "wasm32"))]
    strategy_thread: thread::JoinHandle<()>,

    #[cfg(target_arch = "wasm32")]
    strategy: StrategyRuntime<T>,
}

impl<T: StackType + Send + Sync + FiveLimitIntervalBasis> RunningStrategy<T> {
    #[cfg(not(target_arch = "wasm32"))]
    fn start(
        time: Instant,
        index: usize,
        config: StrategyConfig<T>,
        adaptor_inner: Arc<ConcreteLocks<T>>,
    ) -> Self {
        let (to_strategy_tx, to_strategy_rx) = mpsc::channel();

        let strategy_thread = thread::spawn(move || {
            let mut strategy = StrategyRuntime::from_config(&config);

            strategy.start(time, adaptor_inner.clone());

            while let Ok(msg) = to_strategy_rx.recv() {
                strategy.process_message(msg, adaptor_inner.clone());
            }
        });

        Self {
            index,
            strategy_thread,
            to_strategy_tx,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn start(
        time: Instant,
        index: usize,
        config: StrategyConfig<T>,
        adaptor_inner: Arc<ConcreteLocks<T>>,
    ) -> Self {
        let mut strategy = StrategyRuntime::from_config(&config);

        strategy.start(time, adaptor_inner);

        Self { index, strategy }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn send(&self, msg: ToStrategy<T>) {
        let _ = self.to_strategy_tx.send(msg);
    }

    #[cfg(target_arch = "wasm32")]
    fn send(&mut self, msg: ToStrategy<T>, locks: Arc<ConcreteLocks<T>>) {
        self.strategy.process_message(msg, locks);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn stop(self, time: Instant) -> usize {
        let _ = self.to_strategy_tx.send(ToStrategy::Stop { time });

        self.strategy_thread
            .join()
            .unwrap_or_else(|_| panic!("Could not join running strategy"));

        self.index
    }

    #[cfg(target_arch = "wasm32")]
    fn stop(mut self, time: Instant, locks: Arc<ConcreteLocks<T>>) -> usize {
        self.strategy
            .process_message(ToStrategy::Stop { time }, locks);

        self.index
    }
}

pub struct ProcessFromStrategy<T>
where
    T: StackType + Send + Sync + FiveLimitIntervalBasis,
{
    current_strategy: Option<RunningStrategy<T>>,

    /// The `Option` is only there in order to use Option::take, which allows the "consuming" style
    /// of the [OrderedLocks] functions. This is facilitated by [ProcessFromStrategy::on_adaptor]
    adaptor: Option<ProcessAdaptor<T, Zero>>,
}

impl<T> ProcessFromStrategy<T>
where
    T: StackType + Send + Sync + FiveLimitIntervalBasis + HasFundamental + HasOvertone,
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
    fn send_to_strategy(&mut self, msg: ToStrategy<T>) {
        if let Some(strategy) = &mut self.current_strategy {
            #[cfg(not(target_arch = "wasm32"))]
            {
                strategy.send(msg);
            }

            #[cfg(target_arch = "wasm32")]
            {
                let locks = unsafe { self.adaptor.as_ref().unwrap().inner_arc() };

                strategy.send(msg, locks);
            }
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
        }
    }

    /// Returns the index of the previously running strategy, if any.
    fn stop(&mut self, time: Instant) -> Option<usize> {
        if let Some(rs) = self.current_strategy.take() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                Some(rs.stop(time))
            }

            #[cfg(target_arch = "wasm32")]
            {
                let locks = unsafe { self.adaptor.as_ref().unwrap().inner_arc() };

                Some(rs.stop(time, locks))
            }
        } else {
            None
        }
    }

    /// Will do a restart if there's already a strategy running
    fn start(&mut self, time: Instant, index: usize) {
        if self.current_strategy.is_some() {
            self.stop(time);
        }

        take_replace(&mut self.adaptor, |adaptor| {
            adaptor.active_strategy(|strat, adaptor| {
                self.current_strategy =
                    Some(RunningStrategy::start(time, index, strat.clone(), unsafe {
                        adaptor.inner_arc()
                    }));
            })
        });

        self.send(FromProcess::StartedStrategy {
            index: Some(index),
            time,
        });
    }

    /// Will start strategy 0 if there's no running strategy at the moment.
    fn restart(&mut self, time: Instant) {
        let index = self.stop(time).unwrap_or(0);
        self.start(time, index);
    }
}

impl<T> ReceiveMsg<ToProcess<T>> for ProcessFromStrategy<T>
where
    T: StackType + fmt::Debug + Send + Sync + FiveLimitIntervalBasis + HasOvertone + HasFundamental,
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
