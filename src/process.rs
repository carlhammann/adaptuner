use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use std::{fmt, sync::mpsc};

use crate::{
    interval::Stack,
    util::{Dimension, Vector},
};

#[derive(Clone)]
pub enum Neighbourhood {}

pub mod msg {
    use midi_msg::{MidiMsg, ParseError};

    use crate::{
        interval::Semitones,
        util::{Bounded, Dimension},
    };

    use super::Neighbourhood;

    pub enum ToUI {
        MidiParseErr(ParseError),
    }

    pub enum ToBackend {
        ForwardMidi(MidiMsg),
        Retune { note: u8, target: Semitones },
    }

    pub enum ToProcess<T: Dimension> {
        SetNeighboughood(Neighbourhood),
        ToggleTemperament(Bounded<T>),
    }
}

#[derive(Clone)]
pub struct TuningFrame<'a, D: Dimension, T: Dimension> {
    reference_stack: Stack<'a, D, T>,
    reference_key: u8,
    neighbourhood: Neighbourhood,
    active_temperaments: Vector<T, bool>,
}

pub struct Pattern {}

impl Pattern {
    fn fit(&self, active_notes: &[bool; 128]) -> Option<u8> {
        todo!()
    }
}

pub struct Config<'a> {
    patterns: &'a [Pattern],
    minimum_age: u64, // microseconds
}

pub struct State<'a, D: Dimension, T: Dimension> {
    current: TuningFrame<'a, D, T>,
    old: TuningFrame<'a, D, T>,
    birthday: u64, // microseconds
    active_notes: [bool; 128],

    sustain: bool,

    config: Config<'a>,
    // incoming: mpsc::Receiver<msg::ToProcess<T>>,
    // to_ui: mpsc::Sender<msg::ToUI>,
    // to_backend: mpsc::Sender<msg::ToBackend>,
}

pub trait ProcessState<T: Dimension> {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToProcess<T>,
        to_backend: &mpsc::Sender<msg::ToBackend>,
        to_ui: &mpsc::Sender<msg::ToUI>,
    );
    fn handle_midi_msg(
        &mut self,
        time: u64,
        msg: MidiMsg,
        to_backend: &mpsc::Sender<msg::ToBackend>,
        to_ui: &mpsc::Sender<msg::ToUI>,
    );
}

pub fn process<X: ProcessState<T>, T: Dimension>(
    time: u64,
    msg: &[u8],
    state: (
        &mut X,
        mpsc::Receiver<msg::ToProcess<T>>,
        mpsc::Sender<msg::ToBackend>,
        mpsc::Sender<msg::ToUI>,
    ),
) {
    let (state, incoming, to_backend, to_ui) = state;
    for m in incoming.try_iter() {
        state.handle_msg(time, m, &to_backend, &to_ui);
    }
    // TODO use [MidiMsg::from_midi_with_context] here.
    match MidiMsg::from_midi(msg) {
        Err(e) => to_ui.send(msg::ToUI::MidiParseErr(e)).unwrap_or(()),
        Ok((mm, _number_of_bytes_parsed)) => state.handle_midi_msg(time, mm, &to_backend, &to_ui),
    }
}

impl<'a, D, T> State<'a, D, T>
where
    D: Dimension + Copy + fmt::Debug,
    T: Dimension + Copy,
{
    /// Go through all currently active notes and send [Retune][msg::ToBackend::Retune] messages to
    /// to the backend, describing their current tunings.
    fn send_retunes(&self, to_backend: &mpsc::Sender<msg::ToBackend>) {
        for i in 0..128 {
            if self.active_notes[i] {
                let target = stack_from_tuning_frame(&self.current, i as u8).semitones();
                to_backend
                    .send(msg::ToBackend::Retune {
                        target,
                        note: i as u8,
                    })
                    .unwrap()
                //[send] only fails when backend is disconnected. That'd be bad anyway...
            }
        }
    }
}

impl<'a, D, T> ProcessState<T> for State<'a, D, T>
where
    D: Dimension + Clone + Copy + fmt::Debug,
    T: Dimension + Clone + Copy,
{
    fn handle_msg(
        &mut self,
        _time: u64,
        msg: msg::ToProcess<T>,
        to_backend: &mpsc::Sender<msg::ToBackend>,
        _to_ui: &mpsc::Sender<msg::ToUI>,
    ) {
        match msg {
            msg::ToProcess::SetNeighboughood(n) => {
                self.current.neighbourhood = n;
            }
            msg::ToProcess::ToggleTemperament(t) => {
                self.current.active_temperaments[t] = !self.current.active_temperaments[t];
            }
        }
        self.send_retunes(to_backend);
    }

    fn handle_midi_msg(
        &mut self,
        time: u64,
        msg: MidiMsg,
        to_backend: &mpsc::Sender<msg::ToBackend>,
        _to_ui: &mpsc::Sender<msg::ToUI>,
    ) {
        if time - self.birthday >= self.config.minimum_age {
            self.old.clone_from(&self.current);
        }

        match msg {
            MidiMsg::ChannelVoice {
                channel: _,
                msg: ChannelVoiceMsg::NoteOn { note, velocity: _ },
            } => {
                if !self.active_notes[note as usize] {
                    self.active_notes[note as usize] = true;
                    for p in self.config.patterns {
                        match p.fit(&self.active_notes) {
                            None => {}
                            Some(key) => {
                                if key != self.current.reference_key {
                                    self.current.reference_key = key;
                                    self.current.reference_stack =
                                        stack_from_tuning_frame(&self.old, key);
                                    self.send_retunes(to_backend);
                                }
                                break;
                            }
                        }
                    }
                }
            }

            MidiMsg::ChannelVoice {
                channel: _,
                msg: ChannelVoiceMsg::NoteOff { note, velocity: _ },
            } => {
                if !self.sustain {
                    self.active_notes[note as usize] = false;
                }
            }

            MidiMsg::ChannelVoice {
                channel: _,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Hold(value),
                    },
            } => {
                if value == 0 {
                    self.sustain = false;
                }
            }

            _ => {}
        }
        // this [send] only fails if the backend receiver has already been deallocated, but then
        // we're in trouble anyway.
        to_backend.send(msg::ToBackend::ForwardMidi(msg)).unwrap();
    }
}

fn stack_from_tuning_frame<'a, D: Dimension, T: Dimension>(
    frame: &TuningFrame<'a, D, T>,
    key: u8,
) -> Stack<'a, D, T> {
    todo!()
}
