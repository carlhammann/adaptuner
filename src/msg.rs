use midi_msg::MidiMsg;

use crate::{
    interval::Semitones,
    neighbourhood::Neighbourhood,
    util::{
        dimension::{Bounded, Dimension},
        mod12::PitchClass,
    },
};

pub enum ToUI {
    MidiParseErr(midi_msg::ParseError),
}

pub enum ToBackend {
    ForwardMidi {
        msg: MidiMsg,
    },
    RetuneNote {
        note: u8,
        target: Semitones,
    },
    RetuneClass {
        class: PitchClass,
        target: Semitones,
    },
}

pub enum ToProcess<D: Dimension, T: Dimension> {
    SetNeighboughood { neighbourhood: Neighbourhood<D> },
    ToggleTemperament { index: Bounded<T> },
    IncomingMidi { bytes: Vec<u8> },
}
