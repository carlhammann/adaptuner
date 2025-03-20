use std::{hash::Hash, collections::HashSet, time::Duration};

use midi_msg::{Channel, MidiMsg};
use ndarray::Array1;
use num_rational::Ratio;

use crate::interval::{
    base::Semitones,
    stack::Stack,
    stacktype::r#trait::{StackCoeff, StackType},
};

#[derive(Debug, PartialEq, Clone)]
pub enum AfterProcess<T: StackType + Eq + Hash> {
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
        tuning_stack_actual: Array1<Ratio<StackCoeff>>, // TODO: these should be Arc or something
        // similar
        tuning_stack_targets: HashSet<Stack<T>>,
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

    Special {
        code: u8,
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
    Special { code: u8 },
}
