use std::time::Duration;

use midi_msg::{Channel, MidiMsg};

use crate::interval::{
    interval::Semitones, stack::Stack, stacktype::r#trait::StackCoeff,
    stacktype::r#trait::StackType,
};

#[derive(Debug, PartialEq)]
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

impl<T: StackType> Clone for AfterProcess<T> {
    fn clone(&self) -> Self {
        match self {
            AfterProcess::Start => AfterProcess::Start,
            AfterProcess::Stop => AfterProcess::Stop,
            AfterProcess::Reset => AfterProcess::Reset,
            AfterProcess::Notify { line } => AfterProcess::Notify {
                line: line.to_string(),
            },
            AfterProcess::MidiParseErr(e) => AfterProcess::MidiParseErr(e.to_string()),
            AfterProcess::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => AfterProcess::DetunedNote {
                note: *note,
                should_be: *should_be,
                actual: *actual,
                explanation: *explanation,
            },
            AfterProcess::CrosstermEvent(e) => AfterProcess::CrosstermEvent(e.clone()),
            AfterProcess::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                tuning_stack,
            } => AfterProcess::TunedNoteOn {
                channel: *channel,
                note: *note,
                velocity: *velocity,
                tuning: *tuning,
                tuning_stack: tuning_stack.clone(),
            },
            AfterProcess::NoteOff {
                held_by_sustain,
                channel,
                note,
                velocity,
            } => AfterProcess::NoteOff {
                held_by_sustain: *held_by_sustain,
                channel: *channel,
                note: *note,
                velocity: *velocity,
            },
            AfterProcess::Sustain { channel, value } => AfterProcess::Sustain {
                channel: *channel,
                value: *value,
            },
            AfterProcess::ProgramChange { channel, program } => AfterProcess::ProgramChange {
                channel: *channel,
                program: *program,
            },
            AfterProcess::ForwardMidi { msg } => AfterProcess::ForwardMidi { msg: msg.clone() },
            AfterProcess::Retune {
                note,
                tuning,
                tuning_stack,
            } => AfterProcess::Retune {
                note: *note,
                tuning: *tuning,
                tuning_stack: tuning_stack.clone(),
            },
            AfterProcess::SetReference { key, stack } => AfterProcess::SetReference {
                key: *key,
                stack: stack.clone(),
            },
            AfterProcess::Consider { stack } => AfterProcess::Consider {
                stack: stack.clone(),
            },
            AfterProcess::BackendLatency { since_input } => AfterProcess::BackendLatency {
                since_input: *since_input,
            },
        }
    }
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
