use std::marker::PhantomData;

use midi_msg::{Channel, MidiMsg};

use crate::interval::{
    interval::Semitones, stack::Stack, stacktype::r#trait::StackCoeff,
    stacktype::r#trait::StackType,
};

#[derive(Debug, PartialEq)]
pub enum ToUI<T: StackType> {
    Start,
    Stop,

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

    SetReference {
        key: u8,
        stack: Stack<T>,
    },
    Consider {
        stack: Stack<T>,
    },
    TunedNoteOn {
        note: u8,
        tuning: Stack<T>,
    },
    Retune {
        note: u8,
        tuning: Stack<T>,
    },
    NoteOff {
        note: u8,
    },
}

#[derive(Debug)]
pub enum ToBackend {
    Start,
    Stop,
    Reset,

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

pub enum ToProcess<T: StackType> {
    Start,
    Stop,
    Reset,
    // SetNeighboughood { neighbourhood: Neighbourhood },
    ToggleTemperament {
        index: usize,
    },
    IncomingMidi {
        bytes: Vec<u8>,
    },
    Consider {
        coefficients: Vec<StackCoeff>,
        _phantom: PhantomData<T>
    },
}
