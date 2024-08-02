use std::{marker::PhantomData, ops};

use ndarray::Array2;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    interval::Semitones,
    stacktype::r#trait::{StackCoeff, StackType},
};

/// A stack of [Interval]s.
///
/// For every [StackType] `t`, [Stack]s that reference `t` as their [stacktype][Stack::stacktype]
/// describe linear combinations o f the base [intervals][StackType::intervals] specified of `t`,
/// with adjustments due to the [temperaments][StackType::temperaments] specified by `t`.
///
/// * The function [coefficients][Stack::coefficients] returns the coefficients in the linear
/// combination of intervals, i.e. how many of each type of interval the stack contains. This
/// information can be used to determine the "enharmonically correct" name of the note described by
/// the [Stack].
///
/// * Due to the presence of temperaments, the [coefficients][Stack::coefficients] alone _do not
/// suffice_ to compute the size of the composite interval described by a [Stack]. Use
/// [semitones][Stack::semitones] to compute the size the interval described by a [Stack] as a
/// floating-point number of semitones. You can also use
/// [impure_semitones][Stack::impure_semitones] if you're interested in the deviation from
/// the pure note due to temperaments.
///
/// * However, don't use a floating-point comparison with zero to figure out if a [Stack] contains
/// only pure intervals. Use [is_pure][Stack::is_pure] for that purpose.
///
/// Internally, the "temperament error" is tracked exactly (i.e. using only integer arithmetic).
/// This is what enables [is_pure][Stack::is_pure]. Even more importantly, we need that
/// representation for the "rollovers" that happen when a number of tempered intervals add up to
/// pure intervals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack<T: StackType> {
    _phantom: PhantomData<T>,

    /// a `D`-vector of coefficients,  one for each base interval.
    coefficients: Vec<StackCoeff>,

    /// a `D x T`-matrix of "corrections": The colums correspond to temperaments, and the i-th
    /// entry of every column counts how many (fractions of) commas for the i-th interval from that
    /// temperament should be added to the stack.
    corrections: Array2<StackCoeff>,
}

impl<T: StackType> PartialEq for Stack<T> {
    fn eq(&self, other: &Self) -> bool {
        self.coefficients == other.coefficients && self.corrections == other.corrections
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

                for (j, coeff) in self.coefficients.iter_mut().enumerate() {
                    *coeff += quot * comma[j];
                }

                self.corrections[(i, t)] = rem;
            }
        }
    }

    /// Build a stack for a given [StackType]. The logic around `active_temperaments` and
    /// `coefficients` is the same as for [increment][Stack::increment].
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
            coefficients,
            corrections,
        };
        res.normalise();

        res
    }

    pub fn from_pure_interval(index: usize) -> Self {
        let mut res = Self::new_zero();
        res.coefficients[index] = 1;
        res
    }

    pub fn reset_to(&mut self, active_temperaments: &[bool], coefficients: &[StackCoeff]) {
        for (i, c) in coefficients.iter().enumerate() {
            self.coefficients[i] = *c;
            for (t, active) in active_temperaments.iter().enumerate() {
                if *active {
                    self.corrections[[i, t]] = *c;
                }
            }
        }
        self.normalise();
    }

    /// Apply new temperaments, forgetting all currently applied adjustments. In particular, this
    /// may change the [coefficients][Stack::coefficients], if the new temperaments happen to
    /// "temper out" some interval. (This scenario is like the one explained at the documentation
    /// comment for [increment][Stack::increment].)
    pub fn retemper(&mut self, active_temperaments: &[bool]) {
        for (i, &c) in self.coefficients.iter().enumerate() {
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
            coefficients,
            corrections,
        }
    }

    pub fn coefficients(&self) -> &[StackCoeff] {
        &self.coefficients
    }

    pub fn semitones(&self) -> Semitones {
        let mut pure_semitones = 0.0;
        for (i, c) in self.coefficients.iter().enumerate() {
            pure_semitones += (*c as Semitones) * T::intervals()[i].semitones;
        }
        pure_semitones + self.impure_semitones()
    }

    pub fn impure_semitones(&self) -> Semitones {
        let mut res = 0.0;
        for (ix, c) in self.corrections.indexed_iter() {
            res += (*c as Semitones) * T::precomputed_temperings()[ix];
        }
        res
    }

    pub fn is_pure(&self) -> bool {
        for c in &self.corrections {
            if *c != 0 {
                return false;
            }
        }
        true
    }

    /// How many piano keys wide is the the interval described by this stack?
    pub fn key_distance(&self) -> StackCoeff {
        let mut res = 0;
        for (i, &c) in self.coefficients.iter().enumerate() {
            res += c * T::intervals()[i].key_distance as StackCoeff;
        }
        res
    }

    /// Like [increment_at_index][StackType::increment_at_index], but acting on several intervals at
    /// the same time.
    pub fn increment(&mut self, active_temperaments: &[bool], coefficients: &[StackCoeff]) {
        for (i, coeff) in self.coefficients.iter_mut().enumerate() {
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
    ///
    /// - [index] is the index of the kind interval we're adding in the
    /// [intervals][StackType::intervals].
    ///
    /// - [increment] is the number of intervals we add (and may be negative).
    ///
    /// - [active_temperaments] must have length [num_temperaments][StackType::num_temperaments],
    /// and specifies which temperaments should be used while adding the intervals.
    ///
    /// Due to the temperaments, adding a few intervals of one kind may entail changes to more than
    /// one of the [coefficients][Stack::coefficients]. For example, if you're using a temperament
    /// that has quarter-comma meantone fifths, and add five fifths, then you'll effectivel add one
    /// third and one fifth. (This is why I don't allow direct manipulation of the
    /// [coefficients][Stack::coefficients], but provide functions like this one.)
    pub fn increment_at_index(
        &mut self,
        active_temperaments: &[bool],
        index: usize,
        increment: StackCoeff,
    ) {
        self.coefficients[index] += increment;

        for (t, active) in active_temperaments.iter().enumerate() {
            if *active {
                self.corrections[(index, t)] += increment;
            }
        }
        self.normalise();
    }

    /// Add a multiple of one stack to a stack. See the comment at
    /// [increment_at_index][Stack::increment_at_index] for why this warrants its own function.
    pub fn add_mul(&mut self, n: StackCoeff, added: &Self) {
        for (i, c) in added.coefficients.iter().enumerate() {
            self.coefficients[i] += n * c;
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
    /// Read the documentation comment of [increment][Stack::increment], since addition is
    /// implemented following the same logic.
    ///
    /// Also, in many applications, [increment][Stack::increment]ing might be the cheaper option,
    /// because it doesn't require you to construct a second [Stack].
    fn add(mut self, x: P) -> Self {
        for (ix, coeff) in self.coefficients.iter_mut().enumerate() {
            *coeff += x.coefficients[ix];
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
        assert_relative_eq!(s.impure_semitones(), edo12_third_error, max_relative = eps);
        assert_relative_eq!(s.semitones(), third + edo12_third_error, max_relative = eps);

        let s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 0]);
        assert_relative_eq!(s.impure_semitones(), 0.0, max_relative = eps);
        assert_relative_eq!(s.semitones(), third + 2.0 * octave, max_relative = eps);

        let s = Stack::<MockStackType>::new(&[false, true], vec![0, 6, 0]);
        assert_relative_eq!(
            s.impure_semitones(),
            2.0 * quarter_comma,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones(),
            2.0 * octave + third + 2.0 * fifth + 2.0 * quarter_comma,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::new(&[true, false], vec![0, 0, 7]);
        assert_relative_eq!(s.impure_semitones(), edo12_third_error, max_relative = eps);
        assert_relative_eq!(
            s.semitones(),
            7.0 * third + 7.0 * edo12_third_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::new(&[true, true], vec![0, 5, 7]);
        assert_relative_eq!(
            s.impure_semitones(),
            quarter_comma + 5.0 * edo12_fifth_error + edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones(),
            2.0 * third
                + 4.0 * octave
                + fifth
                + quarter_comma
                + 5.0 * edo12_fifth_error
                + edo12_third_error,
            max_relative = eps
        );
    }

    /// This is a white-box test (since it uses private struct fields). I know, I know... but it
    /// helped me understand the implementation.
    #[test]
    fn test_stack_add() {
        let mut s = Stack::<MockStackType>::new(&[false, false], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[false, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 2], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[true, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[false, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![3 + 0, 0 + 2, 1 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [4, 2], [0, 0]]));

        let mut s = Stack::<MockStackType>::new(&[true, true], vec![0, 4, 3]);
        s = s + &Stack::<MockStackType>::new(&[true, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![3 + 1, 0 + 2, 1 + 2]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [6, 2], [2, 0]]));
    }
}
