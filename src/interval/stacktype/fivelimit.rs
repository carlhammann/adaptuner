use std::sync::LazyLock;

use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    base::{Interval, Semitones},
    stacktype::r#trait::{
        FiveLimitIntervalBasis, IntervalBasis, OctavePeriodicIntervalBasis, PeriodicIntervalBasis,
        StackType,
    },
};

use super::r#trait::{FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType};

use crate::interval::{
    stacktype::r#trait::{NamedInterval, StackTypeInitialisationErr},
    temperament::TemperamentDefinition,
};
use adaptuner_core::interval::StackTypeConfig;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TheFiveLimitStackType {}

impl TheFiveLimitStackType {
    /// Creates an independent runtime configuration for this stack type.
    pub fn create_config(
        temperament_definitions: Vec<TemperamentDefinition<Self>>,
        named_intervals: Vec<NamedInterval<Self>>,
    ) -> Result<StackTypeConfig<Self>, StackTypeInitialisationErr> {
        StackTypeConfig::new(temperament_definitions, named_intervals)
    }
}

static INTERVALS: LazyLock<[Interval; 3]> = LazyLock::new(|| {
    [
        Interval {
            name: "octave".into(),
            semitones: 12.0,
            key_distance: 12,
        },
        Interval {
            name: "fifth".into(),
            semitones: 12.0 * (3.0 / 2.0 as Semitones).log2(),
            key_distance: 7,
        },
        Interval {
            name: "third".into(),
            semitones: 12.0 * (5.0 / 4.0 as Semitones).log2(),
            key_distance: 4,
        },
    ]
});

const INTERVAL_POSITIONS: &[(&str, usize)] = &[("octave", 0), ("fifth", 1), ("third", 2)];

impl IntervalBasis for TheFiveLimitStackType {
    fn intervals() -> &'static [Interval] {
        &*INTERVALS
    }

    fn try_period_index() -> Option<usize> {
        Some(0)
    }

    fn interval_positions() -> &'static [(&'static str, usize)] {
        INTERVAL_POSITIONS
    }
}

impl StackType for TheFiveLimitStackType {}

impl FiveLimitIntervalBasis for TheFiveLimitStackType {
    fn octave_index() -> usize {
        0
    }

    fn fifth_index() -> usize {
        1
    }

    fn third_index() -> usize {
        2
    }
}

impl FiveLimitStackType for TheFiveLimitStackType {}

impl PeriodicIntervalBasis for TheFiveLimitStackType {
    fn period_index() -> usize {
        0
    }
}

impl PeriodicStackType for TheFiveLimitStackType {}

impl OctavePeriodicIntervalBasis for TheFiveLimitStackType {}

impl OctavePeriodicStackType for TheFiveLimitStackType {}

#[cfg(test)]
pub mod mock {

    use ndarray::arr1;
    use serde_derive::{Deserialize, Serialize};

    use crate::interval::{
        base::Interval,
        stacktype::r#trait::{
            FiveLimitIntervalBasis, IntervalBasis, OctavePeriodicIntervalBasis,
            PeriodicIntervalBasis, StackType,
        },
    };

    use super::{INTERVALS, INTERVAL_POSITIONS};

    #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
    pub struct MockFiveLimitStackType {}

    impl IntervalBasis for MockFiveLimitStackType {
        fn intervals() -> &'static [Interval] {
            &*INTERVALS
        }

        fn try_period_index() -> Option<usize> {
            Some(0)
        }

        fn interval_positions() -> &'static [(&'static str, usize)] {
            INTERVAL_POSITIONS
        }
    }

    impl StackType for MockFiveLimitStackType {}

    impl FiveLimitIntervalBasis for MockFiveLimitStackType {
        fn octave_index() -> usize {
            0
        }

        fn fifth_index() -> usize {
            1
        }

        fn third_index() -> usize {
            2
        }
    }

    impl PeriodicIntervalBasis for MockFiveLimitStackType {
        fn period_index() -> usize {
            0
        }
    }

    impl OctavePeriodicIntervalBasis for MockFiveLimitStackType {}

    /// Named intervals used by tests that need a configuration.
    pub fn named_intervals(
    ) -> Vec<crate::interval::stacktype::r#trait::NamedInterval<MockFiveLimitStackType>> {
        vec![
            crate::interval::stacktype::r#trait::NamedInterval::new(
                arr1(&[(-7).into(), 12.into(), 0.into()]),
                "pythagorean comma".into(),
                "p".into(),
            ),
            crate::interval::stacktype::r#trait::NamedInterval::new(
                arr1(&[1.into(), 0.into(), (-3).into()]),
                "diesis".into(),
                "d".into(),
            ),
            crate::interval::stacktype::r#trait::NamedInterval::new(
                arr1(&[(-2).into(), 4.into(), (-1).into()]),
                "syntonic comma".into(),
                "s".into(),
            ),
            crate::interval::stacktype::r#trait::NamedInterval::new(
                arr1(&[1.into(), 0.into(), 0.into()]),
                "octave".into(),
                "o".into(),
            ),
        ]
    }
}
