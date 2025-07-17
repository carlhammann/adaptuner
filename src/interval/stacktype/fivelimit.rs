use std::{
    collections::HashMap,
    fmt::Display,
    sync::{LazyLock, OnceLock},
};

use ndarray::{arr1, arr2};
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    base::{Interval, Semitones},
    stacktype::r#trait::{
        FiveLimitIntervalBasis, IntervalBasis, OctavePeriodicIntervalBasis, PeriodicIntervalBasis,
        StackCoeff, StackType,
    },
    temperament::{Temperament, TemperamentDefinition, TemperamentErr},
};

use super::r#trait::{
    CoordinateSystem, FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType,
};

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TheFiveLimitStackType {}

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

static INTERVAL_POSITIONS: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
    let mut m = HashMap::with_capacity(3);
    m.insert("octave".into(), 0);
    m.insert("fifth".into(), 1);
    m.insert("third".into(), 2);
    m
});

static COMMA_SYSTEMS: LazyLock<[CoordinateSystem; 3]> =
    LazyLock::new(|| {
        [
            CoordinateSystem::new(
                "octave, diesis, syntonic comma".into(),
                arr1(&["octaves".into(), "dieses".into(), "syntonic commas".into()]),
                arr1(&['o', 'd', 's']),
                arr2(&[
                    [1.into(), 1.into(), (-2).into()],
                    [0.into(), 0.into(), 4.into()],
                    [0.into(), (-3).into(), (-1).into()],
                ]),
            )
            .unwrap(),
            CoordinateSystem::new(
                "octave, pythagorean comma, syntonic comma".into(),
                arr1(&[
                    "octaves".into(),
                    "pythagorean commas".into(),
                    "syntonic commas".into(),
                ]),
                arr1(&['o', 'p', 's']),
                arr2(&[
                    [1.into(), (-7).into(), (-2).into()],
                    [0.into(), 12.into(), 4.into()],
                    [0.into(), 0.into(), (-1).into()],
                ]),
            )
            .unwrap(),
            CoordinateSystem::new(
                "octave, pythagorean comma, diesis".into(),
                arr1(&[
                    "octaves".into(),
                    "pythagorean commas".into(),
                    "dieses".into(),
                ]),
                arr1(&['o', 'p', 'd']),
                arr2(&[
                    [1.into(), (-7).into(), 1.into()],
                    [0.into(), 12.into(), 0.into()],
                    [0.into(), 0.into(), (-3).into()],
                ]),
            )
            .unwrap(),
        ]
    });

pub static DIESIS_SYNTONIC: usize = 0;
pub static PYTHAGOREAN_SYNTONIC: usize = 1;
pub static PYTHAGOREAN_DIESIS: usize = 2;

static TEMPERAMENTS: OnceLock<Vec<Temperament<StackCoeff>>> = OnceLock::new();

#[derive(Debug)]
pub enum StackTypeInitialisationErr {
    AlreadyInitialised,
    FromTemperamentErr(TemperamentErr),
}

impl Display for StackTypeInitialisationErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StackTypeInitialisationErr::AlreadyInitialised => {
                write!(f, "The stack type was already initialised")
            }
            StackTypeInitialisationErr::FromTemperamentErr(temperament_err) => {
                temperament_err.fmt(f)
            }
        }
    }
}

impl std::error::Error for StackTypeInitialisationErr {}

impl TheFiveLimitStackType {
    pub fn initialise(
        config: &[TemperamentDefinition<TheFiveLimitStackType>],
    ) -> Result<(), StackTypeInitialisationErr> {
        match config.iter().map(|def| def.realize()).collect() {
            Err(e) => Err(StackTypeInitialisationErr::FromTemperamentErr(e)),
            Ok(temperaments) => match TEMPERAMENTS.set(temperaments) {
                Ok(()) => Ok(()),
                Err(_) => Err(StackTypeInitialisationErr::AlreadyInitialised),
            },
        }
    }
}

impl IntervalBasis for TheFiveLimitStackType {
    fn intervals() -> &'static [Interval] {
        &*INTERVALS
    }

    fn try_period_index() -> Option<usize> {
        Some(0)
    }

    fn interval_positions() -> &'static HashMap<String, usize> {
        &*&INTERVAL_POSITIONS
    }
}

impl StackType for TheFiveLimitStackType {
    fn temperaments() -> &'static [Temperament<StackCoeff>] {
        TEMPERAMENTS.get().expect("temperaments not initialised")
    }

    fn correction_systems() -> &'static [CoordinateSystem] {
        &*COMMA_SYSTEMS
    }
}

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
    use std::sync::LazyLock;

    use ndarray::arr2;

    use crate::interval::{
        base::Interval,
        fundamental::HasFundamental,
        stack::Stack,
        stacktype::r#trait::{
            FiveLimitIntervalBasis, OctavePeriodicIntervalBasis, PeriodicIntervalBasis, StackCoeff,
            StackType,
        },
        temperament::Temperament,
    };

    use super::*;

    #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
    pub struct MockFiveLimitStackType {}

    static MOCK_TEMPERAMENTS: LazyLock<[Temperament<StackCoeff>; 2]> = LazyLock::new(|| {
        [
            Temperament::new(
                String::from("equal temperament"),
                arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]).view(),
                arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]).view(),
            )
            .unwrap(),
            Temperament::new(
                String::from("1/4-comma meantone fifths"),
                arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]).view(),
                arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]).view(),
            )
            .unwrap(),
        ]
    });

    impl IntervalBasis for MockFiveLimitStackType {
        fn intervals() -> &'static [Interval] {
            &*INTERVALS
        }

        fn try_period_index() -> Option<usize> {
            Some(0)
        }

        fn interval_positions() -> &'static HashMap<String, usize> {
            &*&INTERVAL_POSITIONS
        }
    }

    impl StackType for MockFiveLimitStackType {
        fn temperaments() -> &'static [Temperament<StackCoeff>] {
            &*MOCK_TEMPERAMENTS
        }

        fn correction_systems() -> &'static [CoordinateSystem] {
            &*COMMA_SYSTEMS
        }
    }

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

    impl HasFundamental for MockFiveLimitStackType {
        fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
            let mut exponents = [0, 0, 0];

            exponents[0] += a.target[Self::octave_index()];
            exponents[1] += a.target[Self::fifth_index()];
            exponents[0] -= a.target[Self::fifth_index()];
            exponents[2] += a.target[Self::third_index()];
            exponents[0] -= a.target[Self::third_index()] * 2;

            exponents[0] -= b.target[Self::octave_index()];
            exponents[1] -= b.target[Self::fifth_index()];
            exponents[0] += b.target[Self::fifth_index()];
            exponents[2] -= b.target[Self::third_index()];
            exponents[0] += b.target[Self::third_index()] * 2;

            for n in exponents.iter_mut() {
                if *n > 0 {
                    *n = 0;
                }
            }

            exponents[0] += exponents[1];
            exponents[0] += exponents[2] * 2;

            b.increment_at_index_pure(Self::octave_index(), exponents[0]);
            b.increment_at_index_pure(Self::fifth_index(), exponents[1]);
            b.increment_at_index_pure(Self::third_index(), exponents[2]);
        }
    }
}

#[cfg(test)]
mod test {
    use super::mock::*;
    use crate::interval::{fundamental::HasFundamental, stack::Stack};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_target_fundamental() {
        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![1, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 1, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![2, 0, 1])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![-1, 2, 0])
            ),
            Stack::from_target(vec![-3, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-3, -1, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![1, -1, 0])
            ),
            Stack::from_target(vec![0, -1, 0])
        );
    }
}
