#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate std;

extern crate alloc;

pub mod interval;
pub mod midi;
pub mod pitch_bend;
pub mod reference;
pub mod util;

pub use interval::{
    key_distance_from_coefficients, semitones_from_actual, semitones_from_target, CoordinateSystem,
    FiveLimitIntervalBasis, FiveLimitStackType, Interval, IntervalBasis, NamedInterval,
    OctavePeriodicIntervalBasis, OctavePeriodicStackType, PeriodicIntervalBasis, PeriodicStackType,
    ScaledAdd, Semitones, Stack, StackCoeff, StackType, StackTypeConfig,
    StackTypeInitialisationErr,
};
pub mod notes;
pub mod pitch_bend_strategy;
pub mod retuning;
