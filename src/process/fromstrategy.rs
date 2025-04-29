use std::{marker::PhantomData, sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg::*, ControlChange::Hold, MidiMsg};

use crate::{
    config,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg,
    process::r#trait::ProcessState,
    strategy::r#trait::Strategy,
};

pub struct State<T: StackType, S: Strategy<T>> {
    strategy: S,
    key_states: [KeyState; 128],
    tunings: [Stack<T>; 128],
    pedal_hold: [bool; 16],
}

impl<T: StackType, S: Strategy<T>> State<T, S> {
    fn handle_midi(
        &mut self,
        time: Instant,
        msg: MidiMsg,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match msg {
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOn { note, .. },
            } => self.handle_note_on(time, note, channel, to_backend),
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOff { note, .. },
            } => self.handle_note_off(time, note, channel, to_backend),
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
                    for msg in self
                        .strategy
                        .note_off(&self.key_states, &mut self.tunings, &off_notes, time)
                        .drain(..)
                    {
                        send_to_backend(msg::AfterProcess::FromStrategy(msg), time);
                    }
                }
            }
            _ => {}
        }

        send_to_backend(msg::AfterProcess::ForwardMidi { msg }, time);
    }

    fn handle_note_on(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        if self.key_states[note as usize].note_on(channel, time) {
            for msg in self
                .strategy
                .note_on(&self.key_states, &mut self.tunings, note, time)
                .drain(..)
            {
                send_to_backend(msg::AfterProcess::FromStrategy(msg), time);
            }
        }
    }

    fn handle_note_off(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        if self.key_states[note as usize].note_off(channel, self.pedal_hold[channel as usize], time)
        {
            for msg in self
                .strategy
                .note_off(&self.key_states, &mut self.tunings, &[note], time)
                .drain(..)
            {
                send_to_backend(msg::AfterProcess::FromStrategy(msg), time);
            }
        }
    }
}

impl<T: StackType, S: Strategy<T>> ProcessState<T> for State<T, S> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());
        match msg {
            msg::ToProcess::IncomingMidi { bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg, to_backend), // TODO: multi-part messages?
                Err(e) => send_to_backend(msg::AfterProcess::MidiParseErr(e.to_string()), time),
            },
            msg::ToProcess::ToStrategy(to_strategy) => todo!(),
            _ => {} //msg::ToProcess::Start => todo!(),
                    //msg::ToProcess::Stop => todo!(),
                    //msg::ToProcess::Reset => todo!(),
        }
    }
}

pub struct Config<T: StackType, S: Strategy<T>, SC: config::r#trait::Config<S>> {
    pub _phantom: PhantomData<(T, S)>,
    pub strategy_config: SC,
}

impl<T: StackType, S: Strategy<T>, SC: config::r#trait::Config<S>>
    config::r#trait::Config<State<T, S>> for Config<T, S, SC>
{
    fn initialise(config: &Self) -> State<T, S> {
        let now = Instant::now();
        State {
            strategy: <SC as config::r#trait::Config<S>>::initialise(&config.strategy_config),
            key_states: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            pedal_hold: [false; 16],
        }
    }
}
