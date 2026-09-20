use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::LazyLock,
};

use parking_lot::RwLock;

use ndarray::Array2;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    base::{Interval, Semitones},
    fundamental::{HasFundamental, HasOvertone},
    stack::Stack,
    stacktype::r#trait::{
        FiveLimitIntervalBasis, IntervalBasis, OctavePeriodicIntervalBasis, PeriodicIntervalBasis,
        StackCoeff, StackType,
    },
    temperament::{Temperament, TemperamentDefinition},
};

use super::r#trait::{
    CoordinateSystem, FiveLimitStackType, NamedInterval, OctavePeriodicStackType,
    PeriodicStackType, Reloadable, StackTypeInitialisationErr,
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

static NAMED_INTERVALS: RwLock<Vec<NamedInterval<TheFiveLimitStackType>>> = RwLock::new(vec![]);

static COORDINATE_SYSTEMS: RwLock<BTreeMap<usize, (Vec<usize>, CoordinateSystem)>> =
    RwLock::new(BTreeMap::new());

static TEMPERAMENTS: RwLock<Vec<Temperament<StackCoeff>>> = RwLock::new(vec![]);

static TEMPERAMENT_DEFINITIONS: RwLock<Vec<TemperamentDefinition<TheFiveLimitStackType>>> =
    RwLock::new(vec![]);

impl Reloadable for TheFiveLimitStackType {
    fn initialise(
        temperament_definitions: Vec<TemperamentDefinition<TheFiveLimitStackType>>,
        named_intervals: Vec<NamedInterval<TheFiveLimitStackType>>,
    ) -> Result<(), StackTypeInitialisationErr> {
        {
            let mut t = TEMPERAMENTS.write();
            t.clear();
            for def in temperament_definitions.iter() {
                t.push(
                    def.realize()
                        .map_err(StackTypeInitialisationErr::FromTemperamentErr)?,
                );
            }
        }

        {
            let systems = &mut *COORDINATE_SYSTEMS.write();
            let n = named_intervals.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    for k in (j + 1)..n {
                        let mut basis_columnwise = Array2::zeros((3, 3));
                        basis_columnwise
                            .column_mut(0)
                            .assign(&named_intervals[i].coeffs);
                        basis_columnwise
                            .column_mut(1)
                            .assign(&named_intervals[j].coeffs);
                        basis_columnwise
                            .column_mut(2)
                            .assign(&named_intervals[k].coeffs);
                        let _ = CoordinateSystem::new(basis_columnwise).map(|x| {
                            systems.insert(i + j * n + k * n * n, (vec![i, j, k], x));
                        });
                    }
                }
            }
        }

        *TEMPERAMENT_DEFINITIONS.write() = temperament_definitions;
        *NAMED_INTERVALS.write() = named_intervals;

        Ok(())
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
    fn temperaments() -> impl Deref<Target = Vec<Temperament<StackCoeff>>> {
        TEMPERAMENTS.read()
    }

    fn temperament_definitions() -> impl Deref<Target = Vec<TemperamentDefinition<Self>>> {
        TEMPERAMENT_DEFINITIONS.read()
    }

    fn named_intervals() -> impl Deref<Target = Vec<NamedInterval<Self>>> {
        NAMED_INTERVALS.read()
    }

    fn with_coordinate_system<R>(
        basis_indices: &[usize],
        mut f: impl FnMut(Option<&(Vec<usize>, CoordinateSystem)>) -> R,
    ) -> R {
        let i = basis_indices[0].min(basis_indices[1]).min(basis_indices[2]);
        let k = basis_indices[0].max(basis_indices[1]).max(basis_indices[2]);
        let j = basis_indices[0] + basis_indices[1] + basis_indices[2] - i - k;
        let n = Self::named_intervals().len();

        let cs = &*COORDINATE_SYSTEMS.read();
        f(cs.get(&(i + j * n + k * n * n)))
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

impl<T: FiveLimitIntervalBasis> HasFundamental for T {
    fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
        let twos = (b.target[Self::octave_index()]
            - b.target[Self::fifth_index()]
            - 2 * b.target[Self::third_index()])
        .min(
            a.target[Self::octave_index()]
                - a.target[Self::fifth_index()]
                - 2 * a.target[Self::third_index()],
        );
        let threes = (b.target[Self::fifth_index()]).min(a.target[Self::fifth_index()]);
        let fives = (b.target[Self::third_index()]).min(a.target[Self::third_index()]);

        b.increment_at_index_pure(
            Self::octave_index(),
            (twos + threes + 2 * fives) - b.target[Self::octave_index()],
        );
        b.increment_at_index_pure(Self::fifth_index(), threes - b.target[Self::fifth_index()]);
        b.increment_at_index_pure(Self::third_index(), fives - b.target[Self::third_index()]);
    }
}

impl<T: FiveLimitIntervalBasis> HasOvertone for T {
    fn overtone_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
        let twos = (b.target[Self::octave_index()]
            - b.target[Self::fifth_index()]
            - 2 * b.target[Self::third_index()])
        .max(
            a.target[Self::octave_index()]
                - a.target[Self::fifth_index()]
                - 2 * a.target[Self::third_index()],
        );
        let threes = (b.target[Self::fifth_index()]).max(a.target[Self::fifth_index()]);
        let fives = (b.target[Self::third_index()]).max(a.target[Self::third_index()]);

        b.increment_at_index_pure(
            Self::octave_index(),
            (twos + threes + 2 * fives) - b.target[Self::octave_index()],
        );
        b.increment_at_index_pure(Self::fifth_index(), threes - b.target[Self::fifth_index()]);
        b.increment_at_index_pure(Self::third_index(), fives - b.target[Self::third_index()]);
    }
}

#[cfg(test)]
pub mod mock {
    use std::sync::LazyLock;

    use ndarray::{arr1, arr2};

    use crate::interval::{
        base::Interval,
        stacktype::r#trait::{
            FiveLimitIntervalBasis, OctavePeriodicIntervalBasis, PeriodicIntervalBasis, StackCoeff,
            StackType,
        },
        temperament::Temperament,
    };

    use super::*;

    #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
    pub struct MockFiveLimitStackType {}

    static MOCK_TEMPERAMENTS: LazyLock<Vec<Temperament<StackCoeff>>> = LazyLock::new(|| {
        vec![
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

    static MOCK_TEMPERAMENT_DEFINITIONS: LazyLock<
        Vec<TemperamentDefinition<MockFiveLimitStackType>>,
    > = LazyLock::new(|| panic!("no temperament definitions for the mock stack type"));

    static MOCK_NAMED_INTERVALS: LazyLock<Vec<NamedInterval<MockFiveLimitStackType>>> =
        LazyLock::new(|| {
            vec![
                NamedInterval::new(
                    arr1(&[(-7).into(), 12.into(), 0.into()]),
                    "pythagorean comma".into(),
                    "p".into(),
                ),
                NamedInterval::new(
                    arr1(&[1.into(), 0.into(), (-3).into()]),
                    "diesis".into(),
                    "d".into(),
                ),
                NamedInterval::new(
                    arr1(&[(-2).into(), 4.into(), (-1).into()]),
                    "syntonic comma".into(),
                    "s".into(),
                ),
                NamedInterval::new(
                    arr1(&[1.into(), 0.into(), 0.into()]),
                    "octave".into(),
                    "o".into(),
                ),
            ]
        });

    static MOCK_COORDINATE_SYSTEMS: LazyLock<HashMap<usize, (Vec<usize>, CoordinateSystem)>> =
        LazyLock::new(|| {
            let mut systems = HashMap::new();
            let n = MOCK_NAMED_INTERVALS.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    for k in (j + 1)..n {
                        let mut basis_columnwise = Array2::zeros((3, 3));
                        basis_columnwise
                            .column_mut(0)
                            .assign(&MOCK_NAMED_INTERVALS[i].coeffs);
                        basis_columnwise
                            .column_mut(1)
                            .assign(&MOCK_NAMED_INTERVALS[j].coeffs);
                        basis_columnwise
                            .column_mut(2)
                            .assign(&MOCK_NAMED_INTERVALS[k].coeffs);
                        let _ = CoordinateSystem::new(basis_columnwise).map(|x| {
                            systems.insert(i + j * n + k * n * n, (vec![i, j, k], x));
                        });
                    }
                }
            }
            systems
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
        fn temperaments() -> impl Deref<Target = Vec<Temperament<StackCoeff>>> {
            &*MOCK_TEMPERAMENTS
        }

        fn named_intervals() -> impl Deref<Target = Vec<NamedInterval<Self>>> {
            &*MOCK_NAMED_INTERVALS
        }

        fn with_coordinate_system<R>(
            basis_indices: &[usize],
            mut f: impl FnMut(Option<&(Vec<usize>, CoordinateSystem)>) -> R,
        ) -> R {
            let i = basis_indices[0].min(basis_indices[1]).min(basis_indices[2]);
            let k = basis_indices[0].max(basis_indices[1]).max(basis_indices[2]);
            let j = basis_indices[0] + basis_indices[1] + basis_indices[2] - i - k;
            let n = Self::named_intervals().len();
            f(MOCK_COORDINATE_SYSTEMS.get(&(i + j * n + k * n * n)))
        }

        fn temperament_definitions() -> impl Deref<Target = Vec<TemperamentDefinition<Self>>> {
            &*MOCK_TEMPERAMENT_DEFINITIONS
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
}

#[cfg(test)]
mod test {
    use super::mock::*;
    use crate::{
        interval::{
            fundamental::{fundamental_or_overtone, HasFundamental, HasOvertone},
            stack::Stack,
        },
        neighbourhood::{self, Neighbourhood},
    };
    use pretty_assertions::assert_eq;

    #[test]
    fn test_target_fundamental() {
        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![1, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 1, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![2, 0, 1])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![-1, 2, 0])
            ),
            Stack::from_target(vec![-3, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-3, -1, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![1, -1, 0])
            ),
            Stack::from_target(vec![0, -1, 0])
        );
    }

    #[test]
    fn test_target_overtone() {
        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 0, 0])
            ),
            Stack::from_target(vec![1, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![1, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![1, 0, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 1, 0])
            ),
            Stack::from_target(vec![1, 1, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![2, 0, 1])
            ),
            Stack::from_target(vec![2, 0, 1])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![2, 0, 1])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![2, 1, 1])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 1, 0]),
                &Stack::from_target(vec![0, 1, -1])
            ),
            Stack::from_target(vec![2, 1, 0])
        );

        assert_eq!(
            <MockFiveLimitStackType>::overtone(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![-1, 2, 0])
            ),
            Stack::from_target(vec![2, 2, 0])
        );
    }

    #[test]
    fn test_fundamental_or_overtone() {
        let mut n = neighbourhood::Partial::<MockFiveLimitStackType>::new();

        // major third
        n.insert(&Stack::from_target(vec![0, 0, 0]));
        n.insert(&Stack::from_target(vec![0, 0, 1]));
        assert_eq!(
            fundamental_or_overtone(&n),
            (true, Stack::from_target(vec![-2, 0, 0]))
        );
       
        // major chord
        n.insert(&Stack::from_target(vec![0, 1, 0]));
        assert_eq!(
            fundamental_or_overtone(&n),
            (true, Stack::from_target(vec![-2, 0, 0]))
        );

        // minor third
        n.clear();
        n.insert(&Stack::from_target(vec![0, 0, 0]));
        n.insert(&Stack::from_target(vec![0, 1, -1]));
        assert_eq!(
            fundamental_or_overtone(&n),
            (true, Stack::from_target(vec![-2, 0, -1]))
        );

        // minor chord
        n.insert(&Stack::from_target(vec![0, 1, 0]));
        assert_eq!(
            fundamental_or_overtone(&n),
            (false, Stack::from_target(vec![2, 1, 0]))
        );
    }
}
