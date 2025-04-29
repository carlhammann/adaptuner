use std::time::Duration;

use midi_msg::{Channel, MidiMsg};

use crate::interval::{
    base::Semitones,
    stack::Stack,
    stacktype::r#trait::{StackCoeff, StackType},
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

    CrosstermEvent(crossterm::event::Event),

    //NoteOn {
    //    channel: Channel,
    //    note: u8,
    //    velocity: u8,
    //},
    //NoteOff {
    //    held_by_sustain: bool,
    //    channel: Channel,
    //    note: u8,
    //    velocity: u8,
    //},
    //Sustain {
    //    channel: Channel,
    //    value: u8,
    //},
    //ProgramChange {
    //    channel: Channel,
    //    program: u8,
    //},

    ForwardMidi {
        msg: MidiMsg,
    },

    FromStrategy(FromStrategy<T>),

    BackendLatency {
        since_input: Duration,
    },
    
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
}

#[derive(Debug)]
pub enum ToProcess {
    Start,
    Stop,
    Reset,
    IncomingMidi { bytes: Vec<u8> },
    ToStrategy(ToStrategy),
}

#[derive(Debug)]
pub enum ToStrategy {
    Consider { coefficients: Vec<StackCoeff> },
    ToggleTemperament { index: usize },
    //Special { code: u8 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FromStrategy<T:StackType> {
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
    NotifyFit {
        pattern_name: String,
        reference_stack: Stack<T>,
    },
    NotifyNoFit,

    //Special {
    //    code: u8,
    //},

}
