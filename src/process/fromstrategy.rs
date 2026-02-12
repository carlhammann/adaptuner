use std::{
    fmt,
    sync::{mpsc, Arc, RwLock},
    thread,
    time::Instant,
};

use midi_msg::{
    Channel,
    ChannelVoiceMsg::*,
    ControlChange::{Hold, SoftPedal, Sostenuto},
    MidiMsg,
};

use crate::{
    bindable::{Bindings, MidiBindable},
    config::{ExtractConfig, FromConfigAndState, ProcessConfig, StrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, ReceiveMsg, SendMsg, ToProcess, ToStrategy},
    util::readerwriter::{Reader, ReaderWriter},
};

struct RunningStrategy<T: StackType> {
    /// The index in the list of strategies
    index: usize,
    to_strategy_tx: mpsc::Sender<ToStrategy<T>>,
    strategy_thread: thread::JoinHandle<StrategyConfig<T>>,
}

impl<T: StackType + Send + Sync> RunningStrategy<T> {
    fn start(
        time: Instant,
        index: usize,
        config: StrategyConfig<T>,
        from_strategy_tx: mpsc::Sender<FromStrategy<T>>,
        key_states: Reader<[KeyState; 128]>,
        tunings: ReaderWriter<[Stack<T>; 128]>,
    ) -> Self {
        let (to_strategy_tx, to_strategy_rx) = mpsc::channel();

        let strategy_thread = thread::spawn(move || {
            let mut strategy = config.realize(from_strategy_tx, key_states, tunings);

            strategy.receive_msg(ToStrategy::Start { time });

            loop {
                match to_strategy_rx.recv() {
                    Ok(msg) => {
                        let stop = if let ToStrategy::Stop { .. } = &msg {
                            true
                        } else {
                            false
                        };
                        let _ = strategy.receive_msg(msg);
                        if stop {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            strategy.extract_config()
        });

        Self {
            index,
            strategy_thread,
            to_strategy_tx,
        }
    }

    fn stop(self, time: Instant) -> (usize, StrategyConfig<T>) {
        let _ = self.to_strategy_tx.send(ToStrategy::Stop { time });
        (
            self.index,
            self.strategy_thread
                .join()
                .unwrap_or_else(|_| panic!("Could not join running strategy thread")),
        )
    }
}

pub struct ProcessFromStrategy<T: StackType> {
    strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
    key_states: Arc<RwLock<[KeyState; 128]>>,
    tunings: Arc<RwLock<[Stack<T>; 128]>>,
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],

    forward: mpsc::Sender<FromProcess<T>>,
    _message_forward_thread: thread::JoinHandle<mpsc::Receiver<FromStrategy<T>>>,

    current_strategy: Option<RunningStrategy<T>>,
    from_strategy_tx: mpsc::Sender<FromStrategy<T>>,
}

impl<T: StackType + Send + Sync> ProcessFromStrategy<T> {
    pub fn new(
        strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
        forward: mpsc::Sender<FromProcess<T>>,
    ) -> Self {
        if strategies.len() <= 0 {
            panic!("Cannot start process from empty list of strategies");
        }

        let now = Instant::now();
        let key_states = Arc::new(RwLock::new(core::array::from_fn(|_| KeyState::new(now))));
        let tunings = Arc::new(RwLock::new(core::array::from_fn(|_| Stack::new_zero())));

        let (from_strategy_tx, from_strategy_rx) = mpsc::channel();

        let forward_clone = forward.clone();
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
            key_states,
            tunings,
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            forward,
            current_strategy: None {},
            _message_forward_thread: message_forward_thread,
            from_strategy_tx,
        }
    }
}

impl<T: StackType + Send + Sync> ProcessFromStrategy<T> {
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

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: Sostenuto(value),
                    },
            } => {
                if let Some(csi) = self.current_strategy_index() {
                    let (_, ref bindings) = self.strategies[csi];
                    let was_down = self.sostenuto_hold.iter().any(|b| *b);
                    self.sostenuto_hold[channel as usize] = value > 0;
                    let is_down = self.sostenuto_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SostenutoPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SostenutoPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = self.send_to_strategy(ToStrategy::Action { action, time });
                    } else {
                        self.send_msg(untouched_midi());
                    }
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: SoftPedal(value),
                    },
            } => {
                if let Some(csi) = self.current_strategy_index() {
                    let (_, ref bindings) = self.strategies[csi];
                    let was_down = self.soft_hold.iter().any(|b| *b);
                    self.soft_hold[channel as usize] = value > 0;
                    let is_down = self.soft_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SoftPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SoftPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = self.send_to_strategy(ToStrategy::Action { action, time });
                    } else {
                        self.send_msg(untouched_midi());
                    }
                }
            }

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
            if self.key_states.write().unwrap()[note as usize].note_on(channel, time) {
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
            if self.key_states.write().unwrap()[note as usize].note_off(
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
                    let changed = self.key_states.write().unwrap()[i].pedal_off(channel, time);
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
            let (index, config) = rs.stop(time);
            self.strategies[index].0 = config;
            Some(index)
        } else {
            None {}
        }
    }

    /// Will do a restart if there's already a strategy running
    fn start(&mut self, time: Instant, index: usize) {
        if self.current_strategy.is_some() {
            self.stop(time);
        }
        self.current_strategy = Some(RunningStrategy::start(
            time,
            index,
            self.strategies[index].0.clone(),
            self.from_strategy_tx.clone(),
            Reader::new(self.key_states.clone()),
            ReaderWriter::new(self.tunings.clone()),
        ));
    }

    /// Will start strategy 0 if there's no running strategy at the moment.
    fn restart(&mut self, time: Instant) {
        let index = self.stop(time).unwrap_or(0);
        self.start(time, index);
    }
}

impl<T: StackType> SendMsg<FromProcess<T>> for ProcessFromStrategy<T> {
    fn send_msg(&self, msg: FromProcess<T>) -> bool {
        self.forward.send(msg).is_ok()
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
            ToProcess::GetCurrentConfig => {
                let _ = self.send_msg(FromProcess::CurrentConfig(self.extract_config()));
            }
            ToProcess::RestartWithConfig { config, time } => {
                *self =
                    <Self as FromConfigAndState<_, _>>::initialise(config, self.forward.clone());
                self.start(time, 0); // start strategy 0 by default
            }
            ToProcess::RestartWithCurrentConfig { time } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(
                    self.extract_config(),
                    self.forward.clone(),
                );
                self.start(time, 0); // start strategy 0 by default
            }
        }
    }
}

impl<T: StackType> ExtractConfig<ProcessConfig<T>> for ProcessFromStrategy<T> {
    fn extract_config(&self) -> ProcessConfig<T> {
        ProcessConfig {
            strategies: self
                .strategies
                .iter()
                .map(|(s, b)| (s.clone(), b.clone()))
                .collect(),
        }
    }
}

impl<T: StackType + Send + Sync> FromConfigAndState<ProcessConfig<T>, mpsc::Sender<FromProcess<T>>>
    for ProcessFromStrategy<T>
{
    fn initialise(config: ProcessConfig<T>, forward: mpsc::Sender<FromProcess<T>>) -> Self {
        let ProcessConfig { strategies } = config;
        Self::new(strategies, forward)
    }
}
