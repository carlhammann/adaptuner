use crate::interval::{base::Interval, temperament::Temperament};

/// The type of integer coefficients used in [Stack][crate::interval::stack::Stack]s.
pub type StackCoeff = i64;

/// A description of the [Interval]s and [Temperament]s that may be used in a [Stack][crate::interval::stack::Stack]
pub trait StackType: Copy {
    /// The list of "base" [Interval]s that may be used in a [Stack][crate::interval::stack::Stack]
    /// of this type.
    fn intervals() -> &'static [Interval];

    /// The list of [Temperament]s that may be applied to intervals in a
    /// [Stack][crate::interval::stack::Stack] of this type. The
    /// [dimension][Temperament::dimension] of the temperaments must be the
    /// [StackType::num_intervals].
    fn temperaments() -> &'static [Temperament<StackCoeff>];

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

/// Marker trait for stack types whose period is the octave. This means two things: the frequency
/// ratio is 2:1, and there are 12 notes in that space.
pub trait OctavePeriodicStackType: PeriodicStackType {}
