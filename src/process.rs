use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use std::{fmt, sync::mpsc};

use crate::{
    interval::{Stack, StackCoeff},
    msg,
    neighbourhood::Neighbourhood,
    pattern::{Fit, Pattern},
    util::dimension::{AtLeast, Bounded, Dimension, Vector},
};

#[derive(Clone, Debug)]
pub struct TuningFrame<D: Dimension, T: Dimension> {
    pub reference_stack: Stack<D, T>,
    pub reference_key: u8,
    pub neighbourhood: Neighbourhood<D>,
    pub active_temperaments: Vector<T, bool>,
}

#[derive(Debug)]
pub struct Config<'a> {
    pub patterns: &'a [Pattern],
    pub minimum_age: u64, // microseconds
}

#[derive(Debug)]
pub struct State<'a, D: Dimension, T: Dimension> {
    pub current: TuningFrame<D, T>,
    pub old: TuningFrame<D, T>,
    pub birthday: u64, // microseconds
    pub active_notes: [bool; 128],

    pub sustain: bool,

    pub config: Config<'a>,
}

pub trait ProcessState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(u64, msg::ToBackend)>,
        to_ui: &mpsc::Sender<msg::ToUI>,
    );
}

impl<'a, D, T> State<'a, D, T>
where
    D: AtLeast<1> + Copy + fmt::Debug,
    T: Dimension + Copy,
{
    /// Go through all currently active notes and send [RetuneNote][msg::ToBackend::RetuneNote] messages to
    /// to the backend, describing their current tunings.
    fn send_retunes(&self, to_backend: &mpsc::Sender<(u64, msg::ToBackend)>, time: u64) {
        for i in 0..128 {
            if self.active_notes[i] {
                let target = stack_from_tuning_frame(&self.current, i as u8).semitones();
                to_backend
                    .send((
                        time,
                        msg::ToBackend::RetuneNote {
                            target,
                            note: i as u8,
                        },
                    ))
                    .unwrap_or(())
            }
        }
    }
}

impl<'a, D, T> ProcessState<D, T> for State<'a, D, T>
where
    D: AtLeast<1> + Clone + Copy + fmt::Debug,
    T: Dimension + Clone + Copy,
{
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(u64, msg::ToBackend)>,
        to_ui: &mpsc::Sender<msg::ToUI>,
    ) {
        match msg {
            msg::ToProcess::SetNeighboughood { neighbourhood: n } => {
                self.current.neighbourhood = n;
                self.send_retunes(&to_backend, time);
            }
            msg::ToProcess::ToggleTemperament { index: t } => {
                self.current.active_temperaments[t] = !self.current.active_temperaments[t];
                self.send_retunes(&to_backend, time);
            }
            msg::ToProcess::IncomingMidi { bytes } => {
                self.handle_midi_msg(time, &bytes, to_backend, to_ui);
            }
        }
    }
}

impl<'a, D, T> State<'a, D, T>
where
    D: Dimension + AtLeast<1> + fmt::Debug + Copy,
    T: Dimension + Copy,
{
    fn handle_midi_msg(
        &mut self,
        time: u64,
        bytes: &Vec<u8>,
        to_backend: &mpsc::Sender<(u64, msg::ToBackend)>,
        to_ui: &mpsc::Sender<msg::ToUI>,
    ) {
        if time - self.birthday >= self.config.minimum_age {
            self.old.clone_from(&self.current); // TODO: clone-free option?
        }

        match MidiMsg::from_midi(&bytes) {
            Err(e) => to_ui.send(msg::ToUI::MidiParseErr(e)).unwrap_or(()),
            Ok((msg, _number_of_bytes_parsed)) => {
                match msg {
                    MidiMsg::ChannelVoice {
                        channel: _,
                        msg: ChannelVoiceMsg::NoteOn { note, velocity: _ },
                    } => {
                        if !self.active_notes[note as usize] {
                            self.active_notes[note as usize] = true;
                            for p in self.config.patterns {
                                match p.fit(&self.active_notes, 0) {
                                    Fit { reference, next } => {
                                        if next == 128 {
                                            if reference as u8 != self.current.reference_key {
                                                self.current.reference_key = reference as u8;
                                                self.current.reference_stack =
                                                    stack_from_tuning_frame(
                                                        &self.old,
                                                        reference as u8,
                                                    );
                                                self.send_retunes(to_backend, time);
                                            }
                                            break;
                                        }
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
                    } => self.sustain = value != 0,

                    _ => {}
                }
                to_backend
                    .send((time, msg::ToBackend::ForwardMidi { msg }))
                    .unwrap_or(());
            }
        }
    }
}

fn stack_from_tuning_frame<D, T>(frame: &TuningFrame<D, T>, key: u8) -> Stack<D, T>
where
    D: AtLeast<1> + Copy + fmt::Debug,
    T: Dimension + Copy,
{
    let d = key as StackCoeff - frame.reference_key as StackCoeff;
    let (q, r) = (d.div_euclid(12), d.rem_euclid(12));
    let mut coefficients = frame.neighbourhood.coefficients[r as usize].clone();
    coefficients[Bounded::new(0).unwrap()] += q; // unwrap cannot fail here, because of the
                                                 // `AtLeast<1>` bound on `D`
    Stack::new(
        frame.reference_stack.stacktype(),
        &frame.active_temperaments,
        coefficients,
    ) + &frame.reference_stack
}
