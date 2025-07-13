use std::collections::HashMap;

use crate::interval::{base::Interval, temperament::Temperament};

/// The type of integer coefficients used in [Stack][crate::interval::stack::Stack]s.
pub type StackCoeff = i64;

pub trait IntervalBasis: Copy {
    fn intervals() -> &'static [Interval];

    /// Often, there's a "periodicity" in the intervals (like the octave). This function should
    /// return the index of that interval, if it exists.
    ///
    /// This interval doesn't have a logically special status, but knowing it may help in
    /// generating more user-friendly note names, animations etc.
    fn try_period_index() -> Option<usize>;

    /// Convenience: the length of the list returned by [intervals][IntervalBasis::intervals].
    fn num_intervals() -> usize {
        Self::intervals().len()
    }

    /// Convenience: At which position in the list of [IntervalBasis::intervals] is the interval with
    /// the given name?
    fn interval_positions() -> &'static HashMap<String, usize>;
}

/// A description of the [Interval]s and [Temperament]s that may be used in a [Stack][crate::interval::stack::Stack]
pub trait StackType: IntervalBasis {
    /// The list of [Temperament]s that may be applied to intervals in a
    /// [Stack][crate::interval::stack::Stack] of this type. The "dimension" of the temperaments
    /// must be the [IntervalBasis::num_intervals].
    fn temperaments() -> &'static [Temperament<StackCoeff>];

    /// Convenience: the length of the list returned by [temperaments][StackType::temperaments].
    fn num_temperaments() -> usize {
        Self::temperaments().len()
    }
}

pub trait FiveLimitIntervalBasis: IntervalBasis {
    fn octave_index() -> usize;
    fn fifth_index() -> usize;
    fn third_index() -> usize;
}

pub trait FiveLimitStackType: StackType + FiveLimitIntervalBasis {}

pub trait PeriodicIntervalBasis: IntervalBasis {
    fn period_index() -> usize {
        Self::try_period_index().unwrap()
    }

    fn period() -> &'static Interval {
        &Self::intervals()[Self::period_index()]
    }

    fn period_keys() -> u8 {
        Self::period().key_distance
    }
}

pub trait PeriodicStackType: StackType + PeriodicIntervalBasis {}

/// Marker trait for interval bases whose period is the octave. This means two things: the frequency
/// ratio is 2:1, and there are 12 notes in that space.
pub trait OctavePeriodicIntervalBasis: PeriodicIntervalBasis {}

pub trait OctavePeriodicStackType: StackType + OctavePeriodicIntervalBasis {}
