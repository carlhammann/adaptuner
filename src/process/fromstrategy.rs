use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg::*, ControlChange::Hold, MidiMsg};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, HandleMsg, ToProcess},
    strategy::r#trait::Strategy,
};

pub struct ProcessFromStrategy<T: StackType, S: Strategy<T>> {
    strategy: S,
    key_states: [KeyState; 128],
    tunings: [Stack<T>; 128],
    pedal_hold: [bool; 16],
}

impl<T: StackType, S: Strategy<T>> ProcessFromStrategy<T, S> {
    pub fn new(strategy: S) -> Self {
        let now = Instant::now();
        Self {
            strategy,
            key_states: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            pedal_hold: [false; 16],
        }
    }
}

impl<T: StackType, S: Strategy<T>> ProcessFromStrategy<T, S> {
    fn handle_midi(&mut self, time: Instant, msg: MidiMsg, forward: &mpsc::Sender<FromProcess<T>>) {
        match msg {
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOn { note, .. },
            } => self.handle_note_on(time, note, channel, forward),
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOff { note, .. },
            } => self.handle_note_off(time, note, channel, forward),
            MidiMsg::ChannelVoice {
                channel,
                msg: ControlChange {
                    control: Hold(value),
                },
            } => {
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
                    let _success = self.strategy.note_off(
                        &self.key_states,
                        &mut self.tunings,
                        &off_notes,
                        time,
                        forward,
                    );
                }
            }
            _ => {}
        }

        let _ = forward.send(FromProcess::ForwardMidi { msg, time });
    }

    fn handle_note_on(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if self.key_states[note as usize].note_on(channel, time) {
            let _success =
                self.strategy
                    .note_on(&self.key_states, &mut self.tunings, note, time, forward);
        }
    }

    fn handle_note_off(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) {
        if self.key_states[note as usize].note_off(channel, self.pedal_hold[channel as usize], time)
        {
            let _success =
                self.strategy
                    .note_off(&self.key_states, &mut self.tunings, &[note], time, forward);
        }
    }
}

impl<T: StackType, S: Strategy<T>> HandleMsg<ToProcess<T>, FromProcess<T>>
    for ProcessFromStrategy<T, S>
{
    fn handle_msg(&mut self, msg: ToProcess<T>, forward: &mpsc::Sender<FromProcess<T>>) {
        match msg {
            ToProcess::IncomingMidi { time, bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg, forward), // TODO: multi-part messages?
                Err(e) => {
                    let _ = forward.send(FromProcess::MidiParseErr(e.to_string()));
                }
            },
            ToProcess::Stop => {}
            ToProcess::ToStrategy(msg) => {
                let _success =
                    self.strategy
                        .handle_msg(&self.key_states, &mut self.tunings, msg, forward);
            }
        }
    }
}
