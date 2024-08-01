use ndarray::ArrayView2;

use crate::interval::{
    interval::{Interval, Semitones},
    temperament::Temperament,
};

/// The type of integer coefficients used in [Stack][crate::interval::stack::Stack]s.
pub type StackCoeff = i32;

/// A description of the [Interval]s and [Temperament]s that may be used in a [Stack][crate::interval::stack::Stack]
pub trait StackType : Copy{
    /// The list of "base" [Interval]s that may be used in a [Stack][crate::interval::stack::Stack]
    /// of this type.
    fn intervals() -> &'static [Interval];

    /// The list of [Temperament]s that may be applied to intervals in a
    /// [Stack][crate::interval::stack::Stack] of this type. The
    /// [dimension][Temperament::dimension] of the temperaments must be the
    /// [StackType::num_intervals].
    fn temperaments() -> &'static [Temperament<StackCoeff>];

    /// A computation saver: If `num_intervals == d` and `num_temperaments == t`, this will be a `d
    /// x t` matrix of precomputed adjustments. Each column contains the (fractions) of commas (in
    /// the sense of [Temperament]s) for one temperament.
    fn precomputed_temperings() -> ArrayView2<'static, Semitones>;

    /// Convenience: the length of the list returned by [intervals][StackType::intervals].
    fn num_intervals() -> usize {
        Self::intervals().len()
    }

    /// Convenience: the length of the list returned by [temperaments][StackType::temperaments].
    fn num_temperaments() -> usize {
        Self::temperaments().len()
    }
}

pub trait FiveLimitStackType: StackType {
    fn octave_index() -> usize;
    fn fifth_index() -> usize;
    fn third_index() -> usize;
}

pub trait PeriodicStackType: StackType {
    fn period_index() -> usize;

    fn period() -> &'static Interval {
        &Self::intervals()[Self::period_index()]
    }

    fn period_keys() -> u8 {
        Self::period().key_distance
    }
}

pub trait OctavePeriodicStackType: PeriodicStackType {}
