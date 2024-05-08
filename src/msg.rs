use midi_msg::{MidiMsg, ParseError};

use crate::{
    interval::Semitones,
    neighbourhood::Neighbourhood,
    util::dimension::{Bounded, Dimension},
};

pub enum ToUI {
    MidiParseErr(ParseError),
}

pub enum ToBackend {
    ForwardMidi { msg: MidiMsg, time: u64 },
    Retune { note: u8, target: Semitones },
}

pub enum ToProcess<D: Dimension, T: Dimension> {
    SetNeighboughood(Neighbourhood<D>),
    ToggleTemperament(Bounded<T>),
}
