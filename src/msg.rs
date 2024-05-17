use midi_msg::{Channel, MidiMsg};

use crate::{
    interval::Semitones,
    neighbourhood::Neighbourhood,
    util::{
        dimension::{Bounded, Dimension},
        mod12::PitchClass,
    },
};

#[derive(Debug, PartialEq)]
pub enum ToUI {
    MidiParseErr(midi_msg::ParseError),
    Event(crossterm::event::Event),
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
}

#[derive(Debug)]
pub enum ToBackend {
    TunedNoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        tuning: Semitones,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        velocity: u8,
    },
    Sustain {
        channel: Channel,
        value: u8,
   },
    ProgramChange {
        channel: Channel,
        program: u8,
    },
    Retune {
        note: u8,
        tuning: Semitones,
    },
    ForwardMidi {
        msg: MidiMsg,
    },
}

pub enum ToProcess<D: Dimension, T: Dimension> {
    SetNeighboughood { neighbourhood: Neighbourhood<D> },
    ToggleTemperament { index: Bounded<T> },
    IncomingMidi { bytes: Vec<u8> },
}
