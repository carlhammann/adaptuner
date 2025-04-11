use std::{marker::PhantomData, ops};

use ndarray::{Array1, ArrayView1};
use num_rational::Ratio;
use num_traits::Zero;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    base::Semitones,
    stacktype::r#trait::{StackCoeff, StackType},
};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Stack<T: StackType> {
    _phantom: PhantomData<T>,
    pub target: Array1<StackCoeff>,
    pub actual: Array1<Ratio<StackCoeff>>,
}

pub trait ScaledAdd<S> {
    fn scaled_add<P: ops::Deref<Target = Self>>(&mut self, scalar: S, other: P);
}

impl<T: StackType> ScaledAdd<StackCoeff> for Stack<T> {
    fn scaled_add<P: ops::Deref<Target = Stack<T>>>(&mut self, scalar: StackCoeff, other: P) {
        self.target.scaled_add(scalar, &other.target);
        self.actual
            .scaled_add(Ratio::from_integer(scalar), &other.actual);
    }
}

impl<T: StackType> Stack<T> {
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
    pub fn from_target(target: Vec<StackCoeff>) -> Self {
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

    pub fn from_temperaments_and_target(
        active_temperaments: &[bool],
        coefficients: Vec<StackCoeff>,
    ) -> Self {
        let mut actual =
            Array1::from_shape_fn(coefficients.len(), |i| Ratio::from_integer(coefficients[i]));
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                temperament.add_adjustment(ArrayView1::from(&coefficients), actual.view_mut());
            }
        }
        Stack {
            _phantom: PhantomData,
            target: coefficients.into(),
            actual,
        }
    }

    pub fn new_zero() -> Self {
        Stack {
            _phantom: PhantomData,
            target: Array1::zeros(T::num_intervals()),
            actual: Array1::zeros(T::num_intervals()),
        }
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

    pub fn target_coefficients(&self) -> ArrayView1<StackCoeff> {
        self.target.view()
    }

    pub fn actual_coefficients(&self) -> ArrayView1<Ratio<StackCoeff>> {
        self.actual.view()
    }

    pub fn increment_at_index(
        &mut self,
        active_temperaments: &[bool],
        interval_index: usize,
        increment: StackCoeff,
    ) {
        self.target[interval_index] += increment;
        self.actual[interval_index] += increment;
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                self.actual.scaled_add(
                    Ratio::from_integer(increment),
                    &temperament.comma(interval_index),
                );
            }
        }
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

    pub fn semitones(&self) -> Semitones {
        let mut res = 0.0;
        for (i, &c) in self.actual.iter().enumerate() {
            let (n, d) = c.into_raw();
            res += T::intervals()[i].semitones * n as Semitones / d as Semitones;
        }
        res
    }

    /// If the zero stack corresponds to middle C, return the "fractional MIDI note number"
    /// described by this stack.
    pub fn absolute_semitones(&self) -> Semitones {
        self.semitones() + 60.0
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

    pub fn key_distance(&self) -> StackCoeff {
        let mut res = 0;
        for (i, &c) in self.target.iter().enumerate() {
            res += T::intervals()[i].key_distance as StackCoeff * c;
        }
        res
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

    pub fn retemper(&mut self, active_temperaments: &[bool]) {
        self.actual.zip_mut_with(&self.target, |l, r| {
            *l = Ratio::from_integer(*r);
        });
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                temperament.add_adjustment(self.target.view(), self.actual.view_mut());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    type MockStackType = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

    #[test]
    fn test_semitones() {
        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number

        // all four combinations of temperaments for a single third:
        let s =
            Stack::<MockStackType>::from_temperaments_and_target(&[false, false], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, true], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        // all four combinations of temperaments for a single fifth:
        let s =
            Stack::<MockStackType>::from_temperaments_and_target(&[false, false], vec![0, 1, 0]);
        assert_relative_eq!(s.semitones(), fifth, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 1, 0]);
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

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 1, 0]);
        assert_relative_eq!(s.semitones(), 7.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_fifth_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, true], vec![0, 1, 0]);
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
        let octave = 12.0;
        //let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;

        let eps = 0.00000000001; // just an arbitrary small number

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 4, 0]);
        assert_relative_eq!(s.semitones(), 2.0 * octave + third, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            4.0 * quarter_comma_down,
            max_relative = eps
        );
        assert!(s.is_pure());
        assert!(!s.is_target());

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 0, 4]);
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
}
