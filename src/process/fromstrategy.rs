use std::{fmt, sync::mpsc, time::Instant};

use midi_msg::{
    Channel,
    ChannelVoiceMsg::*,
    ControlChange::{Hold, SoftPedal, Sostenuto},
    MidiMsg,
};

use crate::{
    bindable::{Bindable, Bindings},
    config::ExtendedStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, HandleMsg, ToProcess, ToStrategy},
    strategy::r#trait::{Strategy, StrategyAction},
};

pub struct ProcessFromStrategy<T: StackType + 'static> {
    strategies: Vec<(Box<dyn Strategy<T>>, Bindings<StrategyAction>)>,
    templates: &'static [ExtendedStrategyConfig<T>],
    curr_strategy_index: usize,
    key_states: [KeyState; 128],
    tunings: [Stack<T>; 128],
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],
}

impl<T: StackType> ProcessFromStrategy<T> {
    pub fn new(
        strategies: Vec<(Box<dyn Strategy<T>>, Bindings<StrategyAction>)>,
        templates: &'static [ExtendedStrategyConfig<T>],
    ) -> Self {
        let now = Instant::now();
        Self {
            strategies,
            templates,
            curr_strategy_index: 0,
            key_states: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
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
                let (ref mut strategy, ref bindings) = self.strategies[self.curr_strategy_index];
                let was_down = self.sostenuto_hold.iter().any(|b| *b);
                self.sostenuto_hold[channel as usize] = value > 0;
                let is_down = self.sostenuto_hold.iter().any(|b| *b);
                let action = match (was_down, is_down) {
                    (false, true) => bindings.get(Bindable::SostenutoPedalDown),
                    (true, false) => bindings.get(Bindable::SostenutoPedalUp),
                    _ => None {},
                };
                if let Some(&action) = action {
                    let _ = strategy.handle_msg(
                        &self.key_states,
                        &mut self.tunings,
                        ToStrategy::Action { action, time },
                        forward,
                    );
                } else {
                    forward_untouched();
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: SoftPedal(value),
                    },
            } => {
                let (ref mut strategy, ref bindings) = self.strategies[self.curr_strategy_index];
                let was_down = self.soft_hold.iter().any(|b| *b);
                self.soft_hold[channel as usize] = value > 0;
                let is_down = self.soft_hold.iter().any(|b| *b);
                let action = match (was_down, is_down) {
                    (false, true) => bindings.get(Bindable::SoftPedalDown),
                    (true, false) => bindings.get(Bindable::SoftPedalUp),
                    _ => None {},
                };
                if let Some(&action) = action {
                    let _ = strategy.handle_msg(
                        &self.key_states,
                        &mut self.tunings,
                        ToStrategy::Action { action, time },
                        forward,
                    );
                } else {
                    forward_untouched();
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

        if self.key_states[note as usize].note_on(channel, time) {
            match self.strategies[self.curr_strategy_index].0.note_on(
                &self.key_states,
                &mut self.tunings,
                note,
                time,
                forward,
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
                }
                None {} => send_simple_note_on(),
            }
        } else {
            send_simple_note_on();
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
        if self.key_states[note as usize].note_off(channel, self.pedal_hold[channel as usize], time)
        {
            self.strategies[self.curr_strategy_index].0.note_off(
                &self.key_states,
                &mut self.tunings,
                &[note],
                time,
                forward,
            );
        }
        let _ = forward.send(FromProcess::NoteOff {
            channel,
            note,
            velocity,
            time,
        });
    }

    fn handle_pedal_hold(
        &mut self,
        time: Instant,
        channel: Channel,
        value: u8,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if value > 0 {
            self.pedal_hold[channel as usize] = true;
        } else {
            self.pedal_hold[channel as usize] = false;
            let mut off_notes: Vec<u8> = vec![];
            for (note, state) in self.key_states.iter_mut().enumerate() {
                let changed = state.pedal_off(channel, time);
                if changed {
                    off_notes.push(note as u8);
                }
            }
            let _success = self.strategies[self.curr_strategy_index].0.note_off(
                &self.key_states,
                &mut self.tunings,
                &off_notes,
                time,
                forward,
            );
        }
        let _ = forward.send(FromProcess::PedalHold {
            channel,
            value,
            time,
        });
    }

    fn start(&mut self, time: Instant, forward: &mpsc::Sender<FromProcess<T>>) {
        self.strategies[self.curr_strategy_index].0.start(
            &self.key_states,
            &mut self.tunings,
            time,
            forward,
        );
        let _ = forward.send(FromProcess::SwitchToStrategy {
            index: self.curr_strategy_index,
        });
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
                let _success = self.strategies[self.curr_strategy_index].0.handle_msg(
                    &self.key_states,
                    &mut self.tunings,
                    msg,
                    forward,
                );
            }
            ToProcess::SwitchToStrategy { index, time } => {
                self.curr_strategy_index = index;
                let _ = forward.send(FromProcess::SwitchToStrategy { index });
                self.start(time, forward);
            }
            ToProcess::CloneStrategy { index, time } => {
                let (strat, bindings) = &self.strategies[index % self.strategies.len()];
                self.strategies
                    .push((strat.extract_config().realize(), bindings.clone()));
                self.curr_strategy_index = self.strategies.len() - 1;
                let _ = forward.send(FromProcess::SwitchToStrategy {
                    index: self.curr_strategy_index,
                });
                self.start(time, forward);
            }
            ToProcess::AddStrategyFromTemplate { index, time } => {
                let conf = &self.templates[index];
                self.strategies
                    .push((conf.config.clone().realize(), conf.bindings.clone()));
                self.curr_strategy_index = self.strategies.len() - 1;
                let _ = forward.send(FromProcess::SwitchToStrategy {
                    index: self.curr_strategy_index,
                });
                self.start(time, forward);
            }
            ToProcess::DeleteStrategy { index, time } => {
                // the next two lines must follow exactly the same logic as [crate::gui::strategy]
                self.strategies.remove(index);
                self.curr_strategy_index = self.curr_strategy_index.min(self.strategies.len() - 1);
                let _ = forward.send(FromProcess::DeleteStrategy { index });
                self.start(time, forward);
            }
            ToProcess::BindAction { action, bindable } => {
                let (_, bindings) = &mut self.strategies[self.curr_strategy_index];
                bindings.set(bindable, action);
            }
        }
    }
}
