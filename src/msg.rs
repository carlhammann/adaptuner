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

    ForwardMidi {
        msg: MidiMsg,
    },

    FromStrategy(FromStrategy<T>),

    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },


    BackendLatency { since_input: Duration },
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
    Consider {
        /// relative to middle C
        coefficients: Vec<StackCoeff>,
    },
    ToggleTemperament {
        index: usize,
    },
    //Special { code: u8 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FromStrategy<T: StackType> {
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
        /// relative to middle C
        stack: Stack<T>,
    },
    NotifyFit {
        pattern_name: String,
        reference_stack: Stack<T>,
    },
    NotifyNoFit,
}

pub enum ToMidiOut {
    OutgoingMidi { bytes: Vec<u8> },
    Stop,
}

pub enum FromMidiOut {
    EventLatency { since_input: Duration },
}
