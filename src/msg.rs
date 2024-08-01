use std::time::Duration;

use midi_msg::{Channel, MidiMsg};

use crate::interval::{
    interval::Semitones, stack::Stack, stacktype::r#trait::StackCoeff,
    stacktype::r#trait::StackType,
};

#[derive(Debug, PartialEq, Clone)]
pub enum AfterProcess<T: StackType> {
    Start,
    Stop,
    Reset,

    Notify {
        line: String,
    },

    MidiParseErr(String),

    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },

    CrosstermEvent(crossterm::event::Event),

    TunedNoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        tuning: Semitones,
        tuning_stack: Stack<T>,
    },
    NoteOff {
        held_by_sustain: bool,
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
    ForwardMidi {
        msg: MidiMsg,
    },

    Retune {
        note: u8,
        tuning: Semitones,
        tuning_stack: Stack<T>,
    },
    SetReference {
        key: u8,
        stack: Stack<T>,
    },
    Consider {
        stack: Stack<T>,
    },

    BackendLatency {
        since_input: Duration,
    },
}

#[derive(Debug)]
pub enum ToProcess {
    Start,
    Stop,
    Reset,
    IncomingMidi { bytes: Vec<u8> },
    Consider { coefficients: Vec<StackCoeff> },
    ToggleTemperament { index: usize },
}
