use serde_derive::{Deserialize, Serialize};

/// The type of interval sizes measured in equally tempered semitones
pub type Semitones = f64;

/// A "base" interval.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    /// The human-facing name of the interval.
    pub name: String,
    /// The size of the interval in semitones. This is a logarithmic measure: "size in cents
    /// divided by 100".
    pub semitones: Semitones,
    /// The difference of the MIDI key numbers of the upper and lower note in the interval
    pub key_distance: u8,
}
