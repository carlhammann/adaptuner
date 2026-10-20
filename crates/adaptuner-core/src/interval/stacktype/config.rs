use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use ndarray::Array2;

use super::r#trait::{
    CoordinateSystem, IntervalBasis, NamedInterval, StackCoeff, StackTypeInitialisationErr,
};
use crate::interval::temperament::{Temperament, TemperamentDefinition};

/// Runtime configuration for a stack type.
///
/// This configuration owns all runtime data required for temperament
/// calculations, named intervals, and coordinate-system conversions.
///
/// It does not rely on global state or synchronization primitives.
pub struct StackTypeConfig<T: IntervalBasis> {
    _phantom: PhantomData<T>,
    temperament_definitions: Vec<TemperamentDefinition<T>>,
    temperaments: Vec<Temperament<StackCoeff>>,
    named_intervals: Vec<NamedInterval<T>>,
    coordinate_systems: BTreeMap<Vec<usize>, (Vec<usize>, CoordinateSystem)>,
}

impl<T: IntervalBasis> StackTypeConfig<T> {
    /// Constructs a configuration from temperament definitions and named intervals.
    ///
    /// Temperaments are realized and all valid coordinate systems are
    /// precomputed during construction.
    pub fn new(
        temperament_definitions: Vec<TemperamentDefinition<T>>,
        named_intervals: Vec<NamedInterval<T>>,
    ) -> Result<Self, StackTypeInitialisationErr> {
        let mut temperaments = Vec::with_capacity(temperament_definitions.len());

        for definition in &temperament_definitions {
            temperaments.push(
                definition
                    .realize()
                    .map_err(StackTypeInitialisationErr::FromTemperamentErr)?,
            );
        }

        let dimension = T::num_intervals();

        for (index, named_interval) in named_intervals.iter().enumerate() {
            let actual = named_interval.coeffs.len();

            if actual != dimension {
                return Err(StackTypeInitialisationErr::InvalidNamedIntervalDimension {
                    index,
                    expected: dimension,
                    actual,
                });
            }
        }

        let mut coordinate_systems = BTreeMap::new();

        if named_intervals.len() >= dimension {
            let mut combinations = Vec::new();

            collect_combinations(
                named_intervals.len(),
                dimension,
                0,
                &mut Vec::with_capacity(dimension),
                &mut combinations,
            );

            for indices in combinations {
                let mut basis = Array2::zeros((dimension, dimension));

                for (column, &index) in indices.iter().enumerate() {
                    basis
                        .column_mut(column)
                        .assign(&named_intervals[index].coeffs);
                }

                match CoordinateSystem::new(basis) {
                    Ok(coordinate_system) => {
                        coordinate_systems.insert(indices.clone(), (indices, coordinate_system));
                    }
                    Err(crate::util::lu::LUErr::MatrixDegenerate) => {
                        // Singular bases do not define a valid coordinate system.
                        // Skipping them is expected.
                    }
                    Err(err) => {
                        return Err(StackTypeInitialisationErr::FromCoordinateSystemErr(err));
                    }
                }
            }
        }

        Ok(Self {
            _phantom: PhantomData,
            temperament_definitions,
            temperaments,
            named_intervals,
            coordinate_systems,
        })
    }

    pub fn temperaments(&self) -> &[Temperament<StackCoeff>] {
        &self.temperaments
    }

    pub fn temperament_definitions(&self) -> &[TemperamentDefinition<T>] {
        &self.temperament_definitions
    }

    pub fn named_intervals(&self) -> &[NamedInterval<T>] {
        &self.named_intervals
    }

    pub fn num_temperaments(&self) -> usize {
        self.temperaments.len()
    }

    pub fn num_named_intervals(&self) -> usize {
        self.named_intervals.len()
    }

    /// Calls `f` with a coordinate system for the supplied basis indices,
    /// if those indices describe a valid basis.
    ///
    /// The order of indices in the stored coordinate system is canonicalized.
    pub fn with_coordinate_system<R>(
        &self,
        basis_indices: &[usize],
        mut f: impl FnMut(Option<&(Vec<usize>, CoordinateSystem)>) -> R,
    ) -> R {
        let mut key = basis_indices.to_vec();
        key.sort_unstable();

        let coordinate_system = self.coordinate_systems.get(&key);
        f(coordinate_system)
    }
}

/// Collects all combinations of `dimension` indices from `0..n`.
fn collect_combinations(
    n: usize,
    dimension: usize,
    start: usize,
    current: &mut Vec<usize>,
    output: &mut Vec<Vec<usize>>,
) {
    if current.len() == dimension {
        output.push(current.clone());
        return;
    }

    let remaining = dimension - current.len();

    if remaining > n.saturating_sub(start) {
        return;
    }

    for index in start..=n - remaining {
        current.push(index);

        collect_combinations(n, dimension, index + 1, current, output);

        current.pop();
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};
    use ndarray::arr1;
    use num_rational::Ratio;
    use std::sync::LazyLock;

    use super::*;
    use crate::interval::base::Interval;

    #[derive(Clone, Copy, Debug)]
    struct TestBasis;

    static INTERVALS: LazyLock<[Interval; 2]> = LazyLock::new(|| {
        [
            Interval {
                name: "First".into(),
                semitones: 1.0,
                key_distance: 1,
            },
            Interval {
                name: "Second".into(),
                semitones: 2.0,
                key_distance: 2,
            },
        ]
    });

    impl IntervalBasis for TestBasis {
        fn intervals() -> &'static [Interval] {
            &*INTERVALS
        }

        fn try_period_index() -> Option<usize> {
            None
        }

        fn interval_positions() -> &'static [(&'static str, usize)] {
            &[]
        }
    }

    fn named_interval(coeffs: &[i64], name: &str) -> NamedInterval<TestBasis> {
        NamedInterval::new(
            arr1(
                &coeffs
                    .iter()
                    .map(|&coefficient| Ratio::from_integer(coefficient))
                    .collect::<Vec<_>>(),
            ),
            name.into(),
            name.into(),
        )
    }

    #[test]
    fn initializes_with_valid_named_intervals() {
        let config = StackTypeConfig::<TestBasis>::new(
            vec![],
            vec![
                named_interval(&[1, 0], "First"),
                named_interval(&[0, 1], "Second"),
            ],
        )
        .unwrap();

        assert_eq!(config.num_temperaments(), 0);
        assert_eq!(config.num_named_intervals(), 2);

        config.with_coordinate_system(&[0, 1], |coordinate_system| {
            assert!(coordinate_system.is_some());
        });
    }

    #[test]
    fn rejects_named_intervals_with_invalid_dimensions() {
        let result =
            StackTypeConfig::<TestBasis>::new(vec![], vec![named_interval(&[1], "Invalid")]);

        assert!(matches!(
            result,
            Err(StackTypeInitialisationErr::InvalidNamedIntervalDimension {
                index: 0,
                expected: 2,
                actual: 1,
            })
        ));
    }

    #[test]
    fn skips_singular_coordinate_systems() {
        let config = StackTypeConfig::<TestBasis>::new(
            vec![],
            vec![
                named_interval(&[1, 0], "First"),
                named_interval(&[2, 0], "Dependent"),
                named_interval(&[0, 1], "Second"),
            ],
        )
        .unwrap();

        // These two intervals are linearly dependent.
        config.with_coordinate_system(&[0, 1], |coordinate_system| {
            assert!(coordinate_system.is_none());
        });

        // These intervals form a valid basis.
        config.with_coordinate_system(&[0, 2], |coordinate_system| {
            assert!(coordinate_system.is_some());
        });
    }

    #[test]
    fn coordinate_system_lookup_is_order_independent() {
        let config = StackTypeConfig::<TestBasis>::new(
            vec![],
            vec![
                named_interval(&[1, 0], "First"),
                named_interval(&[0, 1], "Second"),
            ],
        )
        .unwrap();

        config.with_coordinate_system(&[1, 0], |coordinate_system| {
            assert!(coordinate_system.is_some());
        });
    }

    #[test]
    fn returns_none_for_unknown_coordinate_system() {
        let config = StackTypeConfig::<TestBasis>::new(
            vec![],
            vec![
                named_interval(&[1, 0], "First"),
                named_interval(&[0, 1], "Second"),
            ],
        )
        .unwrap();

        config.with_coordinate_system(&[0, 5], |coordinate_system| {
            assert!(coordinate_system.is_none());
        });
    }
}
