pub mod base;
pub mod fundamental;
pub mod stack;
pub mod stacktype;
pub mod temperament;
pub mod temperament_serde;

pub use base::{Interval, Semitones};
pub use stack::{
    key_distance_from_coefficients, semitones_from_actual, semitones_from_target, ScaledAdd, Stack,
};
pub use stacktype::config::StackTypeConfig;
pub use stacktype::r#trait::{
    CoordinateSystem, FiveLimitIntervalBasis, FiveLimitStackType, IntervalBasis, NamedInterval,
    OctavePeriodicIntervalBasis, OctavePeriodicStackType, PeriodicIntervalBasis, PeriodicStackType,
    StackCoeff, StackType, StackTypeInitialisationErr,
};
