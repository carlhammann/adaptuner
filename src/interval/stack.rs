use std::{marker::PhantomData, ops};

use ndarray::Array2;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    interval::Semitones,
    stacktype::r#trait::{StackCoeff, StackType},
};

/// A stack of [Interval]s.
///
/// For every [StackType] `T`, elements of `[Stack]<T>`
/// describe linear combinations of the base [intervals][StackType::intervals] specified by `T`,
/// with adjustments due to the [temperaments][StackType::temperaments] specified by `T`.
///
/// * The function [coefficients][Stack::coefficients] returns the coefficients in the linear
/// combination of intervals, i.e. how many of each type of interval the stack contains. This
/// information can be used to determine the "enharmonically correct" name of the note described by
/// the [Stack].
///
/// * Due to the presence of temperaments, the [coefficients][Stack::coefficients] alone _do not
/// suffice_ to compute the size of the composite interval described by a [Stack]. Use
/// [relative_semitones][Stack::relative_semitones] to compute the size the interval described by a
/// [Stack] as a floating-point number of semitones. You can also use
/// [semitones_away_from_pure][Stack::semitones_away_from_pure] if you're interested in the
/// deviation from the pure note due to temperaments.
///
/// * However, don't use a floating-point comparison with zero to figure out if a [Stack] contains
/// only pure intervals. Use [is_pure][Stack::is_pure] and
/// [tempered_to_pure][Stack::tempered_to_pure] for that purpose.
///
/// Internally, the "temperament error" is tracked exactly (i.e. using only integer arithmetic).
/// This is what enables [is_pure][Stack::is_pure]. Even more importantly, we need that
/// representation for the "rollovers" that happen when a number of tempered intervals add up to
/// pure intervals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack<T: StackType> {
    _phantom: PhantomData<T>,

    /// the `tempered_coefficients` (and `corrections`) keep track of intervals *after* temperaments
    /// were applied; this tracks the intervals *before* temperaments.
    coefficients: Vec<StackCoeff>,

    /// a `D`-vector of coefficients,  one for each base interval.
    tempered_coefficients: Vec<StackCoeff>,

    /// a `D x T`-matrix of "corrections": The colums correspond to temperaments, and the i-th
    /// entry of every column counts how many (fractions of) commas for the i-th interval from that
    /// temperament should be added to the stack.
    corrections: Array2<StackCoeff>,
}

impl<T: StackType> PartialEq for Stack<T> {
    fn eq(&self, other: &Self) -> bool {
        self.tempered_coefficients == other.tempered_coefficients
            && self.corrections == other.corrections
    }
}

impl<T: StackType> Stack<T> {
    /// private: the absolute value of `corrections[[i,t]]` must be less than the
    /// [denominator][Temperament::denominator] of the `t`-th temperament for interval `i`.
    /// This invariant is enforced by all functions we expose.
    fn normalise(&mut self) {
        for (t, temper) in T::temperaments().iter().enumerate() {
            for (i, _) in T::intervals().iter().enumerate() {
                let comma = temper.comma(i);
                let denominator = temper.denominator(i);

                let quot = self.corrections[[i, t]] / denominator;
                let rem = self.corrections[[i, t]] % denominator;

                for (j, coeff) in self.tempered_coefficients.iter_mut().enumerate() {
                    *coeff += quot * comma[j];
                }

                self.corrections[(i, t)] = rem;
            }
        }
    }

    /// Build a stack for a given [StackType].
    ///
    /// Since this function is called quite often, I don't check the following invariants on the
    /// arguments:
    ///
    /// - `active_temperaments.len() == stacktype.temperaments().len()`
    /// - `coefficients.len() == stacktype.intervals().len()`
    pub fn new(active_temperaments: &[bool], coefficients: Vec<StackCoeff>) -> Stack<T> {
        let corrections =
            Array2::from_shape_fn((coefficients.len(), active_temperaments.len()), |(i, t)| {
                if active_temperaments[t] {
                    coefficients[i]
                } else {
                    0
                }
            });

        let mut res = Stack {
            _phantom: PhantomData,
            tempered_coefficients: coefficients.clone(),
            coefficients,
            corrections,
        };
        res.normalise();

        res
    }

    pub fn from_pure_interval(index: usize) -> Self {
        let mut res = Self::new_zero();
        res.tempered_coefficients[index] = 1;
        res.coefficients[index] = 1;
        res.normalise();
        res
    }

    pub fn reset_to(&mut self, active_temperaments: &[bool], coefficients: &[StackCoeff]) {
        for (i, &c) in coefficients.iter().enumerate() {
            self.coefficients[i] = c;
            self.tempered_coefficients[i] = c;
            for (t, active) in active_temperaments.iter().enumerate() {
                if *active {
                    self.corrections[[i, t]] = c;
                }
            }
        }
        self.normalise();
    }

    pub fn reset_to_zero(&mut self) {
        for c in self.coefficients.iter_mut() {
            *c = 0;
        }
        for c in self.tempered_coefficients.iter_mut() {
            *c = 0;
        }
        for c in self.corrections.iter_mut() {
            *c = 0;
        }
    }

    /// Apply new temperaments, forgetting all currently applied adjustments.
    pub fn retemper(&mut self, active_temperaments: &[bool]) {
        for (i, &c) in self.coefficients.iter().enumerate() {
            self.tempered_coefficients[i] = c;
            for (t, &active) in active_temperaments.iter().enumerate() {
                self.corrections[[i, t]] = if active { c } else { 0 };
            }
        }
        self.normalise();
    }

    pub fn new_zero() -> Stack<T> {
        let coefficients = vec![0; T::num_intervals()];
        let corrections = Array2::zeros((T::num_intervals(), T::num_temperaments()));
        Stack {
            _phantom: PhantomData,
            tempered_coefficients: coefficients.clone(),
            coefficients,
            corrections,
        }
    }

    pub fn coefficients(&self) -> &[StackCoeff] {
        &self.coefficients
    }

    pub fn relative_semitones(&self) -> Semitones {
        let mut res = 0.0;
        for (i, c) in self.tempered_coefficients.iter().enumerate() {
            res += (*c as Semitones) * T::intervals()[i].semitones;
        }
        for (ix, c) in self.corrections.indexed_iter() {
            res += (*c as Semitones) * T::precomputed_temperings()[ix];
        }
        res
    }

    /// If `self` is a stack above C4, this will return the "fractional MIDI note number" described
    /// by this stack
    pub fn absolute_semitones(&self) -> Semitones {
        self.relative_semitones() + 60.0
    }

    pub fn semitones_away_from_pure(&self) -> Semitones {
        let mut res = 0.0;
        for (i, c) in self.coefficients.iter().enumerate() {
            res += (*c as Semitones) * T::intervals()[i].semitones;
        }
        self.relative_semitones() - res
    }

    /// True iff the stack describes a pure note and no temperaments are applied to any intervals
    /// in the stack.
    pub fn is_pure(&self) -> bool {
        for (i, &c) in self.coefficients.iter().enumerate() {
            if self.tempered_coefficients[i] != c {
                return false;
            }
        }
        return true;
    }

    /// True iff the stack describes a pure note, but not necessarily the same as described by its
    /// [coefficients][Stack::coefficients]. This may happen when the temperaments just add up
    /// right.
    pub fn tempered_to_pure(&self) -> bool {
        for (_i, &c) in self.corrections.iter().enumerate() {
            if 0 != c {
                return false;
            }
        }
        return true;
    }

    /// How many piano keys wide is the the interval described by this stack?
    pub fn key_distance(&self) -> StackCoeff {
        let mut res = 0;
        for (i, &c) in self.tempered_coefficients.iter().enumerate() {
            res += c * T::intervals()[i].key_distance as StackCoeff;
        }
        res
    }

    /// If the zero stack describes MIDI key C4, which key does `self` describe?
    pub fn key_number(&self) -> StackCoeff {
        60 + self.key_distance()
    }

    /// Like [increment_at][Stack::increment_at], but acting on several intervals at
    /// the same time.
    pub fn increment(&mut self, active_temperaments: &[bool], coefficients: &[StackCoeff]) {
        for (i, coeff) in self.coefficients.iter_mut().enumerate() {
            *coeff += coefficients[i];
        }
        for (i, coeff) in self.tempered_coefficients.iter_mut().enumerate() {
            *coeff += coefficients[i];
        }

        for (t, active) in active_temperaments.iter().enumerate() {
            if *active {
                for (i, coeff) in coefficients.iter().enumerate() {
                    self.corrections[(i, t)] += coeff;
                }
            }
        }
        self.normalise();
    }

    /// Add or subtract a few intervals to a `Stack`.
    pub fn increment_at(
        &mut self,
        active_temperaments: &[bool],
        index: usize,
        increment: StackCoeff,
    ) {
        self.tempered_coefficients[index] += increment;
        self.coefficients[index] += increment;

        for (t, active) in active_temperaments.iter().enumerate() {
            if *active {
                self.corrections[(index, t)] += increment;
            }
        }
        self.normalise();
    }

    /// Add a multiple of one stack to a stack.
    pub fn add_mul(&mut self, n: StackCoeff, added: &Self) {
        for (i, c) in added.coefficients.iter().enumerate() {
            self.coefficients[i] += n * c;
            self.tempered_coefficients[i] += n * c;
        }
        for (i, c) in added.corrections.indexed_iter() {
            self.corrections[i] += n * c;
        }
        self.normalise()
    }
}

impl<T: StackType, P: ops::Deref<Target = Stack<T>>> ops::Add<P> for Stack<T> {
    type Output = Self;

    /// Addition of [Stack]s is a bit more involved than one might think at first glance: It
    /// involves more than adding corresponding [coefficients][Stack::coefficients], because of the
    /// effect of [temperaments][StackType::temperaments].
    ///
    /// Also, in many applications, [increment][Stack::increment]ing might be the cheaper option,
    /// because it doesn't require you to construct a second [Stack].
    fn add(mut self, x: P) -> Self {
        for (ix, coeff) in self.coefficients.iter_mut().enumerate() {
            *coeff += x.coefficients[ix];
        }
        for (ix, coeff) in self.tempered_coefficients.iter_mut().enumerate() {
            *coeff += x.tempered_coefficients[ix];
        }
        for (ix, corr) in self.corrections.indexed_iter_mut() {
            *corr += x.corrections[ix];
        }
        self.normalise();

        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
    use approx::*;
    use ndarray::arr2;

    type MockStackType = ConcreteFiveLimitStackType;

    #[test]
    fn test_stack_semitones() {
        let octave = 12.0;
        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma = 3.0 * (80.0 / 81.0 as Semitones).log2();
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number. I don't care about
                                 // extreme numerical stbility.

        let s = Stack::<MockStackType>::new(&[true, false], vec![0, 0, 1]);
        assert_relative_eq!(
            s.semitones_away_from_pure(),
            edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.relative_semitones(),
            third + edo12_third_error,
            max_relative = eps
        );
        assert!(!s.tempered_to_pure());

        let s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 0]);
        assert_relative_eq!(
            s.semitones_away_from_pure(),
            4.0 * quarter_comma,
            max_relative = eps
        );
        assert_relative_eq!(
            s.relative_semitones(),
            third + 2.0 * octave,
            max_relative = eps
        );
        assert!(s.tempered_to_pure());

        let s = Stack::<MockStackType>::new(&[false, true], vec![0, 6, 0]);
        assert_relative_eq!(
            s.semitones_away_from_pure(),
            6.0 * quarter_comma,
            max_relative = eps
        );
        assert_relative_eq!(
            s.relative_semitones(),
            2.0 * octave + third + 2.0 * fifth + 2.0 * quarter_comma,
            max_relative = eps
        );
        assert!(!s.tempered_to_pure());

        let s = Stack::<MockStackType>::new(&[true, false], vec![0, 0, 7]);
        assert_relative_eq!(
            s.semitones_away_from_pure(),
            7.0 * edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.relative_semitones(),
            7.0 * third + 7.0 * edo12_third_error,
            max_relative = eps
        );
        assert!(!s.tempered_to_pure());

        let s = Stack::<MockStackType>::new(&[true, true], vec![0, 5, 7]);
        assert_relative_eq!(
            s.semitones_away_from_pure(),
            5.0 * quarter_comma + 5.0 * edo12_fifth_error + 7.0 * edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.relative_semitones(),
            2.0 * third
                + 4.0 * octave
                + fifth
                + quarter_comma
                + 5.0 * edo12_fifth_error
                + edo12_third_error,
            max_relative = eps
        );
        assert!(!s.tempered_to_pure());
    }

    /// This is a white-box test (since it uses private struct fields). I know, I know... but it
    /// helped me understand the implementation.
    #[test]
    fn test_stack_add() {
        let mut s = Stack::<MockStackType>::new(&[false, false], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.tempered_coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.tempered_coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.tempered_coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 2], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[true, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.tempered_coefficients, vec![3 + 0, 0 + 2, 1 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [4, 2], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[true, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[true, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.tempered_coefficients, vec![3 + 1, 0 + 2, 1 + 2]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [6, 2], [2, 0]]));
    }
}
