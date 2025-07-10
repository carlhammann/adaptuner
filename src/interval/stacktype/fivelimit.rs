use std::sync::{LazyLock, OnceLock};

use ndarray::{arr2, Array2};
use num_traits::Zero;
use serde::{Deserialize, Serialize};

use crate::interval::{
    base::{Interval, Semitones},
    stacktype::r#trait::{
        FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType,
    },
    temperament::{Temperament, TemperamentErr},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct FiveLimitCoeffs {
    #[serde(default)]
    #[serde(skip_serializing_if = "StackCoeff::is_zero")]
    octaves: StackCoeff,
    #[serde(default)]
    #[serde(skip_serializing_if = "StackCoeff::is_zero")]
    fifths: StackCoeff,
    #[serde(default)]
    #[serde(skip_serializing_if = "StackCoeff::is_zero")]
    thirds: StackCoeff,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FiveLimitTemperamentEquation {
    tempered: FiveLimitCoeffs,
    pure: FiveLimitCoeffs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FiveLimitTemperamentDefinition {
    pub name: String,
    pub equations: [FiveLimitTemperamentEquation; 3],
}

impl FiveLimitTemperamentDefinition {
    pub fn from_temperament_definition(def: &TemperamentDefinition) -> Self {
        Self {
            name: def.name.clone(),
            equations: [
                FiveLimitTemperamentEquation {
                    tempered: FiveLimitCoeffs {
                        octaves: def.tempered[(0, 0)],
                        fifths: def.tempered[(0, 1)],
                        thirds: def.tempered[(0, 2)],
                    },
                    pure: FiveLimitCoeffs {
                        octaves: def.pure[(0, 0)],
                        fifths: def.pure[(0, 1)],
                        thirds: def.pure[(0, 2)],
                    },
                },
                FiveLimitTemperamentEquation {
                    tempered: FiveLimitCoeffs {
                        octaves: def.tempered[(1, 0)],
                        fifths: def.tempered[(1, 1)],
                        thirds: def.tempered[(1, 2)],
                    },
                    pure: FiveLimitCoeffs {
                        octaves: def.pure[(1, 0)],
                        fifths: def.pure[(1, 1)],
                        thirds: def.pure[(1, 2)],
                    },
                },
                FiveLimitTemperamentEquation {
                    tempered: FiveLimitCoeffs {
                        octaves: def.tempered[(2, 0)],
                        fifths: def.tempered[(2, 1)],
                        thirds: def.tempered[(2, 2)],
                    },
                    pure: FiveLimitCoeffs {
                        octaves: def.pure[(2, 0)],
                        fifths: def.pure[(2, 1)],
                        thirds: def.pure[(2, 2)],
                    },
                },
            ],
        }
    }

    pub fn to_temperament_definition(self) -> TemperamentDefinition {
        TemperamentDefinition {
            name: self.name,
            tempered: arr2(&[
                [
                    self.equations[0].tempered.octaves,
                    self.equations[0].tempered.fifths,
                    self.equations[0].tempered.thirds,
                ],
                [
                    self.equations[1].tempered.octaves,
                    self.equations[1].tempered.fifths,
                    self.equations[1].tempered.thirds,
                ],
                [
                    self.equations[2].tempered.octaves,
                    self.equations[2].tempered.fifths,
                    self.equations[2].tempered.thirds,
                ],
            ]),
            pure: arr2(&[
                [
                    self.equations[0].pure.octaves,
                    self.equations[0].pure.fifths,
                    self.equations[0].pure.thirds,
                ],
                [
                    self.equations[1].pure.octaves,
                    self.equations[1].pure.fifths,
                    self.equations[1].pure.thirds,
                ],
                [
                    self.equations[2].pure.octaves,
                    self.equations[2].pure.fifths,
                    self.equations[2].pure.thirds,
                ],
            ]),
        }
    }
}

pub struct TemperamentDefinition {
    pub name: String,
    pub tempered: Array2<StackCoeff>,
    pub pure: Array2<StackCoeff>,
}

pub enum TemperamentInitialisationErr {
    AlreadyInitialised,
    FromTemperamentErr(TemperamentErr),
}

pub fn realise_temperaments(
    definitions: &[TemperamentDefinition],
) -> Result<Vec<Temperament<StackCoeff>>, TemperamentInitialisationErr> {
    let f = |def: &TemperamentDefinition| match Temperament::new(
        def.name.clone(),
        def.tempered.view(),
        def.pure.view(),
    ) {
        Ok(t) => Ok(t),
        Err(e) => Err(TemperamentInitialisationErr::FromTemperamentErr(e)),
    };
    definitions.iter().map(f).collect()
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
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

static TEMPERAMENTS: OnceLock<Vec<Temperament<StackCoeff>>> = OnceLock::new();

impl TheFiveLimitStackType {
    pub fn initialise(
        config: &[TemperamentDefinition],
    ) -> Result<(), TemperamentInitialisationErr> {
        match TEMPERAMENTS.set(realise_temperaments(config)?) {
            Ok(()) => Ok(()),
            Err(_) => Err(TemperamentInitialisationErr::AlreadyInitialised),
        }
    }
}

impl StackType for TheFiveLimitStackType {
    fn intervals() -> &'static [Interval] {
        &*INTERVALS
    }

    fn temperaments() -> &'static [Temperament<StackCoeff>] {
        TEMPERAMENTS.get().expect("temperaments not initialised")
    }
}

impl FiveLimitStackType for TheFiveLimitStackType {
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

impl PeriodicStackType for TheFiveLimitStackType {
    fn period_index() -> usize {
        0
    }
}

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
            FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType,
        },
        temperament::Temperament,
    };

    use super::*;

    #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
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

    impl StackType for MockFiveLimitStackType {
        fn intervals() -> &'static [Interval] {
            &*INTERVALS
        }

        fn temperaments() -> &'static [Temperament<StackCoeff>] {
            &*MOCK_TEMPERAMENTS
        }
    }

    impl FiveLimitStackType for MockFiveLimitStackType {
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

    impl PeriodicStackType for MockFiveLimitStackType {
        fn period_index() -> usize {
            0
        }
    }

    impl OctavePeriodicStackType for MockFiveLimitStackType {}

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
