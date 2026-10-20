use alloc::vec::Vec;
use core::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops,
};

use ndarray::{Array1, ArrayView1};
use num_rational::Ratio;
use num_traits::Zero;

use crate::interval::{
    base::Semitones,
    stacktype::r#trait::{IntervalBasis, StackCoeff},
    StackTypeConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stack<T: IntervalBasis> {
    _phantom: PhantomData<T>,
    pub target: Array1<StackCoeff>,
    pub actual: Array1<Ratio<StackCoeff>>,
}

impl<T: IntervalBasis> Hash for Stack<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.actual.hash(state);
    }
}

pub trait ScaledAdd<S> {
    fn scaled_add<P: ops::Deref<Target = Self>>(&mut self, scalar: S, other: P);
}

impl<T: IntervalBasis> ScaledAdd<StackCoeff> for Stack<T> {
    fn scaled_add<P: ops::Deref<Target = Stack<T>>>(&mut self, scalar: StackCoeff, other: P) {
        self.target.scaled_add(scalar, &other.target);
        self.actual
            .scaled_add(Ratio::from_integer(scalar), &other.actual);
    }
}

/// Like [Stack::key_distance], but for cases when you only have the target coefficients and
/// not a whole [Stack].
pub fn key_distance_from_coefficients<T: IntervalBasis>(
    target: ArrayView1<StackCoeff>,
) -> StackCoeff {
    let mut res = 0;
    for (i, &c) in target.iter().enumerate() {
        res += T::intervals()[i].key_distance as StackCoeff * c;
    }
    res
}

/// Like [Stack::target_semitones], but for cases when you only have the target coefficients and
/// not a whole [Stack].
pub fn semitones_from_target<T: IntervalBasis>(target: ArrayView1<StackCoeff>) -> Semitones {
    let mut res = 0.0;
    for (i, &c) in target.iter().enumerate() {
        res += T::intervals()[i].semitones * c as Semitones;
    }
    res
}

/// Like [Stack::semitones], but for cases when you only have what would be the [Stack::actual]
/// coefficients and not a whole [Stack].
pub fn semitones_from_actual<T: IntervalBasis>(actual: ArrayView1<Ratio<StackCoeff>>) -> Semitones {
    let mut res = 0.0;
    for (i, &c) in actual.iter().enumerate() {
        let (n, d) = c.into_raw();
        res += T::intervals()[i].semitones * n as Semitones / d as Semitones;
    }
    res
}

impl<T: IntervalBasis> Stack<T> {
    pub fn from_target_and_actual(
        target: Array1<StackCoeff>,
        actual: Array1<Ratio<StackCoeff>>,
    ) -> Self {
        Stack {
            _phantom: PhantomData,
            target,
            actual,
        }
    }

    /// actual will be initialised to the same as target
    pub fn from_target<V: Into<Array1<StackCoeff>>>(target: V) -> Self {
        let target = target.into();
        let actual = Array1::from_shape_fn(target.len(), |i| Ratio::from_integer(target[i]));
        Stack {
            _phantom: PhantomData,
            target: target.into(),
            actual,
        }
    }

    pub fn raw(self) -> (Array1<StackCoeff>, Array1<Ratio<StackCoeff>>) {
        (self.target, self.actual)
    }

    pub fn new_zero() -> Self {
        Stack {
            _phantom: PhantomData,
            target: Array1::zeros(T::num_intervals()),
            actual: Array1::zeros(T::num_intervals()),
        }
    }

    pub fn scale(&mut self, scalar: StackCoeff) {
        self.target.iter_mut().for_each(|c| *c *= scalar);
        self.actual.iter_mut().for_each(|c| *c *= scalar);
    }

    pub fn from_pure_interval(interval_index: usize, multiplier: StackCoeff) -> Self {
        let mut target = Array1::zeros(T::num_intervals());
        target[interval_index] = multiplier;
        let mut actual = Array1::zeros(T::num_intervals());
        actual[interval_index] = Ratio::from_integer(multiplier);
        Stack {
            _phantom: PhantomData,
            target,
            actual,
        }
    }

    pub fn target_coefficients<'a>(&'a self) -> ArrayView1<'a, StackCoeff> {
        self.target.view()
    }

    pub fn actual_coefficients<'a>(&'a self) -> ArrayView1<'a, Ratio<StackCoeff>> {
        self.actual.view()
    }

    pub fn increment_at_index_pure(&mut self, interval_index: usize, increment: StackCoeff) {
        self.target[interval_index] += increment;
        self.actual[interval_index] += increment;
    }

    pub fn is_target(&self) -> bool {
        for (i, r) in self.actual.iter().enumerate() {
            if !r.is_integer() {
                return false;
            }
            if r.to_integer() != self.target[i] {
                return false;
            }
        }

        true
    }

    /// - `s.is_target()´ implies ´s.is_pure()´, but not vice versa.
    pub fn is_pure(&self) -> bool {
        for r in self.actual.iter() {
            if !r.is_integer() {
                return false;
            }
        }

        true
    }

    /// Size of the interval, described in fractional semitones.
    pub fn semitones(&self) -> Semitones {
        semitones_from_actual::<T>(self.actual.view())
    }

    /// Like [Self::semitones], but for the target note.
    pub fn target_semitones(&self) -> Semitones {
        semitones_from_target::<T>(self.target.view())
    }

    /// If the zero stack corresponds to middle C, return the "fractional MIDI note number"
    /// described by this stack.
    pub fn absolute_semitones(&self, c4_semitones: Semitones) -> Semitones {
        self.semitones() + c4_semitones
    }

    /// Like [Self::absolute_semitones], but for the target note.
    pub fn target_absolute_semitones(&self, c4_semitones: Semitones) -> Semitones {
        self.target_semitones() + c4_semitones
    }

    /// How many fractional semitones higher than the target note is the actual note described by
    /// this stack?
    pub fn semitones_above_target(&self) -> Semitones {
        let mut res = 0.0;
        for (i, &c) in self.target.iter().enumerate() {
            res += T::intervals()[i].semitones * c as Semitones;
        }
        self.semitones() - res
    }

    /// The number of keys on the piano keyboard spanned by the described interval
    pub fn key_distance(&self) -> StackCoeff {
        key_distance_from_coefficients::<T>(self.target.view())
    }

    /// If the zero stack corresponds to middle C, return the MIDI note number of the key that this
    /// stack describes. This uses the [Self::key_distance], so it returns the "enharmonically
    /// correct" key, not the one whose (equally tempered) MIDI note is closest to the actually
    /// sounding note.
    pub fn key_number(&self) -> StackCoeff {
        self.key_distance() + 60
    }

    pub fn reset_to_zero(&mut self) {
        self.target.fill(0);
        self.actual.fill(Ratio::zero());
    }

    pub fn make_pure(&mut self) {
        self.actual.zip_mut_with(&self.target, |l, r| {
            *l = Ratio::from_integer(*r);
        });
    }

    /// Creates a stack using the explicitly supplied temperament configuration.
    pub fn from_temperaments_and_target_with_config(
        config: &StackTypeConfig<T>,
        active_temperaments: &[bool],
        coefficients: Vec<StackCoeff>,
    ) -> Self {
        let mut actual =
            Array1::from_shape_fn(coefficients.len(), |i| Ratio::from_integer(coefficients[i]));

        let temperaments = config.temperaments();

        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &temperaments[t];
                temperament.add_adjustment(ArrayView1::from(&coefficients), actual.view_mut());
            }
        }

        Stack {
            _phantom: PhantomData,
            target: coefficients.into(),
            actual,
        }
    }

    /// Increments an interval using the supplied temperament configuration.
    pub fn increment_at_index_with_config(
        &mut self,
        config: &StackTypeConfig<T>,
        active_temperaments: &[bool],
        interval_index: usize,
        increment: StackCoeff,
    ) {
        self.target[interval_index] += increment;
        self.actual[interval_index] += increment;

        let temperaments = config.temperaments();

        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &temperaments[t];

                self.actual.scaled_add(
                    Ratio::from_integer(increment),
                    &temperament.comma(interval_index),
                );
            }
        }
    }

    /// Recalculates the actual coefficients using the supplied configuration.
    pub fn retemper_with_config(
        &mut self,
        config: &StackTypeConfig<T>,
        active_temperaments: &[bool],
    ) {
        self.actual.zip_mut_with(&self.target, |actual, target| {
            *actual = Ratio::from_integer(*target);
        });

        let temperaments = config.temperaments();

        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                temperaments[t].add_adjustment(self.target.view(), self.actual.view_mut());
            }
        }
    }

    /// Clears all other adjustments and applies one temperament from the supplied configuration.
    pub fn apply_temperament_with_config(
        &mut self,
        config: &StackTypeConfig<T>,
        temperament_index: usize,
    ) {
        self.actual.zip_mut_with(&self.target, |actual, target| {
            *actual = Ratio::from_integer(*target);
        });

        let temperaments = config.temperaments();
        temperaments[temperament_index].add_adjustment(self.target.view(), self.actual.view_mut());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::{vec, vec::Vec};
    use approx::assert_relative_eq;
    use ndarray::{arr1, arr2};
    use std::sync::LazyLock;

    use crate::interval::{
        base::Interval,
        stacktype::config::StackTypeConfig,
        stacktype::r#trait::{
            FiveLimitIntervalBasis, NamedInterval, OctavePeriodicIntervalBasis,
            PeriodicIntervalBasis, StackType,
        },
        temperament::TemperamentDefinition,
    };

    #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
    struct MockStackType;

    static MOCK_INTERVALS: LazyLock<[Interval; 3]> = LazyLock::new(|| {
        [
            Interval {
                name: "octave".into(),
                semitones: 12.0,
                key_distance: 12,
            },
            Interval {
                name: "fifth".into(),
                semitones: 12.0 * (3.0_f64 / 2.0).log2(),
                key_distance: 7,
            },
            Interval {
                name: "third".into(),
                semitones: 12.0 * (5.0_f64 / 4.0).log2(),
                key_distance: 4,
            },
        ]
    });

    static MOCK_TEMPERAMENT_DEFINITIONS: LazyLock<Vec<TemperamentDefinition<MockStackType>>> =
        LazyLock::new(|| {
            vec![
                TemperamentDefinition::new(
                    "equal temperament".into(),
                    arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                    arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
                ),
                TemperamentDefinition::new(
                    "1/4-comma meantone fifths".into(),
                    arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                    arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
                ),
            ]
        });

    static MOCK_NAMED_INTERVALS: LazyLock<Vec<NamedInterval<MockStackType>>> =
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

    fn mock_config() -> StackTypeConfig<MockStackType> {
        StackTypeConfig::new(
            MOCK_TEMPERAMENT_DEFINITIONS.to_vec(),
            MOCK_NAMED_INTERVALS.to_vec(),
        )
        .unwrap()
    }

    impl IntervalBasis for MockStackType {
        fn intervals() -> &'static [Interval] {
            &*MOCK_INTERVALS
        }

        fn try_period_index() -> Option<usize> {
            Some(0)
        }

        fn interval_positions() -> &'static [(&'static str, usize)] {
            &[("octave", 0), ("fifth", 1), ("third", 2)]
        }
    }

    impl StackType for MockStackType {}

    impl FiveLimitIntervalBasis for MockStackType {
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

    impl PeriodicIntervalBasis for MockStackType {
        fn period_index() -> usize {
            0
        }
    }

    impl OctavePeriodicIntervalBasis for MockStackType {}

    #[test]
    fn test_semitones() {
        let config = mock_config();

        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number

        // all four combinations of temperaments for a single third:
        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, false],
            vec![0, 0, 1],
        );
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, true],
            vec![0, 0, 1],
        );
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, false],
            vec![0, 0, 1],
        );
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, true],
            vec![0, 0, 1],
        );
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        // all four combinations of temperaments for a single fifth:
        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, false],
            vec![0, 1, 0],
        );
        assert_relative_eq!(s.semitones(), fifth, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, true],
            vec![0, 1, 0],
        );
        assert_relative_eq!(
            s.semitones(),
            fifth + quarter_comma_down,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            quarter_comma_down,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, false],
            vec![0, 1, 0],
        );
        assert_relative_eq!(s.semitones(), 7.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_fifth_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, true],
            vec![0, 1, 0],
        );
        assert_relative_eq!(
            s.semitones(),
            fifth + quarter_comma_down + edo12_fifth_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_fifth_error + quarter_comma_down,
            max_relative = eps
        );
    }

    #[test]
    fn test_rollovers() {
        let config = mock_config();

        let octave = 12.0;
        //let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;

        let eps = 0.00000000001; // just an arbitrary small number

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, true],
            vec![0, 4, 0],
        );
        assert_relative_eq!(s.semitones(), 2.0 * octave + third, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            4.0 * quarter_comma_down,
            max_relative = eps
        );
        assert!(s.is_pure());
        assert!(!s.is_target());

        let s = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, false],
            vec![0, 0, 4],
        );
        assert_relative_eq!(
            s.semitones(),
            octave + third + edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            4.0 * edo12_third_error,
            max_relative = eps
        );
        assert!(!s.is_pure());
        assert!(!s.is_target());
    }

    #[test]
    fn stack_configuration_instances_are_independent() {
        let config_a = StackTypeConfig::<MockStackType>::new(
            vec![TemperamentDefinition::new(
                "temperament A".into(),
                arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
            )],
            vec![],
        )
        .unwrap();

        let config_b = StackTypeConfig::<MockStackType>::new(
            vec![TemperamentDefinition::new(
                "temperament B".into(),
                arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
            )],
            vec![],
        )
        .unwrap();

        let coefficients = vec![0, 4, 0];

        let stack_a = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config_a,
            &[true],
            coefficients.clone(),
        );

        let stack_b = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config_b,
            &[true],
            coefficients,
        );

        // Die beiden Konfigurationen müssen unabhängig voneinander wirken.
        assert_ne!(stack_a.actual, stack_b.actual);

        // Ein vorhandener Stack kann mit einer anderen Konfiguration
        // neu berechnet werden.
        let mut recalculated = stack_a.clone();
        recalculated.retemper_with_config(&config_b, &[true]);

        assert_eq!(recalculated.actual, stack_b.actual);

        // Die erneute Berechnung mit A liefert wieder das ursprüngliche Ergebnis.
        recalculated.retemper_with_config(&config_a, &[true]);

        assert_eq!(recalculated.actual, stack_a.actual);
    }

    #[test]
    fn increment_with_config_matches_full_recalculation() {
        let config = StackTypeConfig::<MockStackType>::new(
            vec![
                TemperamentDefinition::new(
                    "temperament A".into(),
                    arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                    arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
                ),
                TemperamentDefinition::new(
                    "temperament B".into(),
                    arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                    arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
                ),
            ],
            vec![],
        )
        .unwrap();

        let active = [true, false];

        let mut stack = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &active,
            vec![0, 4, 0],
        );

        stack.increment_at_index_with_config(&config, &active, 1, 1);

        let mut recalculated = stack.clone();
        recalculated.retemper_with_config(&config, &active);

        assert_eq!(stack.target, recalculated.target);
        assert_eq!(stack.actual, recalculated.actual);
    }

    #[test]
    fn apply_temperament_with_config_matches_single_active_temperament() {
        let config = StackTypeConfig::<MockStackType>::new(
            vec![
                TemperamentDefinition::new(
                    "temperament A".into(),
                    arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                    arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
                ),
                TemperamentDefinition::new(
                    "temperament B".into(),
                    arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                    arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
                ),
            ],
            vec![],
        )
        .unwrap();

        let target = vec![0, 4, 0];

        let mut stack = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[true, true],
            target.clone(),
        );

        stack.apply_temperament_with_config(&config, 1);

        let expected = Stack::<MockStackType>::from_temperaments_and_target_with_config(
            &config,
            &[false, true],
            target,
        );

        assert_eq!(stack.target, expected.target);
        assert_eq!(stack.actual, expected.actual);
    }
}
