use ndarray::ArrayView2;

use crate::interval::{
    interval::{Interval, Semitones},
    temperament::Temperament,
};

/// The type of integer coefficients used in [Stack][crate::interval::stack::Stack]s.
pub type StackCoeff = i32;

/// A description of the [Interval]s and [Temperament]s that may be used in a [Stack][crate::interval::stack::Stack]
pub trait StackType {
    /// The list of "base" [Interval]s that may be used in a [Stack][crate::interval::stack::Stack]
    /// of this type.
    fn intervals(&self) -> &[Interval];

    /// The list of [Temperament]s that may be applied to intervals in a
    /// [Stack][crate::interval::stack::Stack] of this type. The
    /// [dimension][Temperament::dimension] of the temperaments must be the
    /// [StackType::num_intervals].
    fn temperaments(&self) -> &[Temperament<StackCoeff>];

    /// A computation saver: If `num_intervals == d` and `num_temperaments == t`, this will be a `d
    /// x t` matrix of precomputed adjustments. Each column contains the (fractions) of commas (in
    /// the sense of [Temperament]s) for one temperament.
    fn precomputed_temperings(&self) -> ArrayView2<Semitones>;

    /// Convenience: the length of the list returned by [intervals][StackType::intervals].
    fn num_intervals(&self) -> usize {
        self.intervals().len()
    }

    /// Convenience: the length of the list returned by [temperaments][StackType::temperaments].
    fn num_temperaments(&self) -> usize {
        self.temperaments().len()
    }
}

pub trait FiveLimitStackType: StackType {
    fn octave_index(&self) -> usize;
    fn fifth_index(&self) -> usize;
    fn third_index(&self) -> usize;
}
