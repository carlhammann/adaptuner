use midi_msg::{Channel, MidiMsg};

use crate::{
    interval::{Semitones, Stack},
    neighbourhood::Neighbourhood,
    util::dimension::{Bounded, Dimension},
};

#[derive(Debug, PartialEq)]
pub enum ToUI<D: Dimension, T: Dimension> {
    Notify {
        line: String,
    },

    MidiParseErr(midi_msg::ParseError),
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },

    CrosstermEvent(crossterm::event::Event),

    SetNeighboughood {
        neighbourhood: Neighbourhood<D>,
    },
    ToggleTemperament {
        index: Bounded<T>,
    },
    SetReference {
        key: u8,
        stack: Stack<D, T>,
    },
    NoteOn {
        note: u8,
    },
    NoteOff {
        note: u8,
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
    ForwardBytes {
        bytes: Vec<u8>,
    },
}

pub enum ToProcess<D: Dimension, T: Dimension> {
    SetNeighboughood { neighbourhood: Neighbourhood<D> },
    ToggleTemperament { index: Bounded<T> },
    IncomingMidi { bytes: Vec<u8> },
}
