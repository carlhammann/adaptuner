use std::{collections::VecDeque, fmt, sync::mpsc, time::Instant};

use midi_msg::{
    Channel,
    ChannelVoiceMsg::*,
    ControlChange::{Hold, SoftPedal, Sostenuto},
    MidiMsg,
};

use crate::{
    bindable::{Bindings, MidiBindable},
    config::{ExtractConfig, FromConfigAndState, ProcessConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, HandleMsg, ToProcess, ToStrategy},
    strategy::r#trait::Strategy,
};

pub struct ProcessFromStrategy<T: StackType> {
    strategies: Vec<(Box<dyn Strategy<T>>, Bindings<MidiBindable>)>,
    curr_strategy_index: Option<usize>,
    key_states: [KeyState; 128],
    tunings: [Stack<T>; 128],
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],
    queue: VecDeque<FromStrategy<T>>,
}

impl<T: StackType> ProcessFromStrategy<T> {
    pub fn new(strategies: Vec<(Box<dyn Strategy<T>>, Bindings<MidiBindable>)>) -> Self {
        let now = Instant::now();
        Self {
            curr_strategy_index: if strategies.len() > 0 {
                Some(0)
            } else {
                None {}
            },
            strategies,
            key_states: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            queue: VecDeque::new(),
        }
    }
}

impl<T: StackType> ProcessFromStrategy<T> {
    fn handle_midi(&mut self, time: Instant, msg: MidiMsg, forward: &mpsc::Sender<FromProcess<T>>) {
        let forward_untouched = || {
            let _ = forward.send(FromProcess::OutgoingMidi {
                bytes: msg.to_midi(),
                time,
            });
        };

        match msg {
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOn { note, velocity },
            } => self.handle_note_on(time, note, channel, velocity, forward),
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOff { note, velocity },
            } => self.handle_note_off(time, note, channel, velocity, forward),
            MidiMsg::ChannelVoice {
                channel,
                msg: ControlChange {
                    control: Hold(value),
                },
            } => self.handle_pedal_hold(time, channel, value, forward),

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: Sostenuto(value),
                    },
            } => {
                if let Some(csi) = self.curr_strategy_index {
                    let (ref mut strategy, ref bindings) = self.strategies[csi];
                    let was_down = self.sostenuto_hold.iter().any(|b| *b);
                    self.sostenuto_hold[channel as usize] = value > 0;
                    let is_down = self.sostenuto_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SostenutoPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SostenutoPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = strategy.handle_msg(
                            &self.key_states,
                            &mut self.tunings,
                            ToStrategy::Action { action, time },
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            let _ = forward.send(FromProcess::FromStrategy(msg));
                        });
                    } else {
                        forward_untouched();
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
                if let Some(csi) = self.curr_strategy_index {
                    let (ref mut strategy, ref bindings) = self.strategies[csi];
                    let was_down = self.soft_hold.iter().any(|b| *b);
                    self.soft_hold[channel as usize] = value > 0;
                    let is_down = self.soft_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SoftPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SoftPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = strategy.handle_msg(
                            &self.key_states,
                            &mut self.tunings,
                            ToStrategy::Action { action, time },
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            let _ = forward.send(FromProcess::FromStrategy(msg));
                        });
                    } else {
                        forward_untouched();
                    }
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg: ProgramChange { program },
            } => {
                let _ = forward.send(FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                });
            }

            _ => forward_untouched(),
        }
    }

    fn handle_note_on(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        velocity: u8,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        let send_simple_note_on = || {
            let _ = forward.send(FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            });
        };

        if let Some(csi) = self.curr_strategy_index {
            if self.key_states[note as usize].note_on(channel, time) {
                match self.strategies[csi].0.note_on(
                    &self.key_states,
                    &mut self.tunings,
                    note,
                    time,
                    &mut self.queue,
                ) {
                    Some((tuning, tuning_stack)) => {
                        let _ = forward.send(FromProcess::TunedNoteOn {
                            channel,
                            note,
                            velocity,
                            tuning,
                            tuning_stack: tuning_stack.clone(),
                            time,
                        });
                        self.queue.drain(..).for_each(|msg| {
                            let _ = forward.send(FromProcess::FromStrategy(msg));
                        });
                    }
                    None {} => {
                        send_simple_note_on();
                        self.queue.clear();
                    }
                }
            } else {
                send_simple_note_on();
            }
        }
    }

    fn handle_note_off(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        velocity: u8,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if let Some(csi) = self.curr_strategy_index {
            if self.key_states[note as usize].note_off(
                channel,
                self.pedal_hold[channel as usize],
                time,
            ) {
                self.strategies[csi].0.note_off(
                    &self.key_states,
                    &mut self.tunings,
                    note,
                    time,
                    &mut self.queue,
                );
                self.queue.drain(..).for_each(|msg| {
                    let _ = forward.send(FromProcess::FromStrategy(msg));
                });
            }
            let _ = forward.send(FromProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_pedal_hold(
        &mut self,
        time: Instant,
        channel: Channel,
        value: u8,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if let Some(csi) = self.curr_strategy_index {
            if value > 0 {
                self.pedal_hold[channel as usize] = true;
            } else {
                self.pedal_hold[channel as usize] = false;
                for i in 0..128 {
                    let changed = self.key_states[i].pedal_off(channel, time);
                    if changed {
                        let _ = self.strategies[csi].0.note_off(
                            &self.key_states,
                            &mut self.tunings,
                            i as u8,
                            time,
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            let _ = forward.send(FromProcess::FromStrategy(msg));
                        });
                    }
                }
            }
            let _ = forward.send(FromProcess::PedalHold {
                channel,
                value,
                time,
            });
        }
    }

    fn start(&mut self, time: Instant, forward: &mpsc::Sender<FromProcess<T>>) {
        if let Some(csi) = self.curr_strategy_index {
            self.strategies[csi].0.start(
                &self.key_states,
                &mut self.tunings,
                time,
                &mut self.queue,
            );
            self.queue.drain(..).for_each(|msg| {
                let _ = forward.send(FromProcess::FromStrategy(msg));
            });
        }
        let _ = forward.send(FromProcess::CurrentStrategyIndex(self.curr_strategy_index));
    }
}

impl<T: StackType + fmt::Debug + 'static> HandleMsg<ToProcess<T>, FromProcess<T>>
    for ProcessFromStrategy<T>
{
    fn handle_msg(&mut self, msg: ToProcess<T>, forward: &mpsc::Sender<FromProcess<T>>) {
        match msg {
            ToProcess::Stop => {}
            ToProcess::Reset { time } => self.start(time, forward),
            ToProcess::Start { time } => self.start(time, forward),
            ToProcess::IncomingMidi { time, bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg, forward), // TODO: multi-part messages?
                Err(e) => {
                    let _ = forward.send(FromProcess::MidiParseErr(e.to_string()));
                }
            },
            ToProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_on(time, note, channel, velocity, forward),
            ToProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_off(time, note, channel, velocity, forward),
            ToProcess::PedalHold {
                channel,
                value,
                time,
            } => self.handle_pedal_hold(time, channel, value, forward),
            ToProcess::ToStrategy(msg) => {
                if let Some(csi) = self.curr_strategy_index {
                    let _success = self.strategies[csi].0.handle_msg(
                        &self.key_states,
                        &mut self.tunings,
                        msg,
                        &mut self.queue,
                    );
                    self.queue.drain(..).for_each(|msg| {
                        let _ = forward.send(FromProcess::FromStrategy(msg));
                    });
                }
            }
            ToProcess::StrategyListAction { action, time } => {
                action.apply_to(
                    |(strat, bind)| (strat.extract_config().realize(), bind.clone()),
                    &mut self.strategies,
                    &mut self.curr_strategy_index,
                );
                self.start(time, forward);
            }
            ToProcess::BindAction { action, bindable } => {
                if let Some(csi) = self.curr_strategy_index {
                    let (_, bindings) = &mut self.strategies[csi];
                    if let Some(action) = action {
                        bindings.insert(bindable, action);
                    } else {
                        bindings.remove(&bindable);
                    }
                }
            }
            ToProcess::GetCurrentConfig => {
                let _ = forward.send(FromProcess::CurrentConfig(self.extract_config()));
            }
            ToProcess::RestartWithConfig { time, config } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(config, ());
                self.start(time, forward);
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
                .map(|(s, b)| (s.extract_config(), b.clone()))
                .collect(),
        }
    }
}

impl<T: StackType, S> FromConfigAndState<ProcessConfig<T>, S> for ProcessFromStrategy<T> {
    fn initialise(config: ProcessConfig<T>, _state: S) -> Self {
        let ProcessConfig { mut strategies } = config;
        Self::new(
            strategies
                .drain(..)
                .map(|(s, b)| (s.realize(), b))
                .collect(),
        )
    }
}
