//! Everything that has to do with (stacks of) intervals, pure or tempered.
use std::{ops, sync::Arc};

use ndarray::{Array2, ArrayView2};
use serde_derive::{Deserialize, Serialize};

mod temperament;
pub use temperament::*;

/// The type of integer coefficients used in [Stack]s
pub type StackCoeff = i32;

/// The type of interval sizes measured in equally tempered semitones
pub type Semitones = f64;

/// A "base" interval.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    /// the human-facing name of the interval.
    pub name: String,
    /// the size of the interval in semitones. This is a logarithmic measure: "size in cents
    /// divided by 100".
    pub semitones: Semitones,
    /// The difference of the MIDI key numbers of the upper and lower note in the interval
    pub key_distance: u8,
}

pub trait StackType {
    fn intervals(&self) -> &[Interval];
    fn temperaments(&self) -> &[Temperament<StackCoeff>];
    fn precomputed_temperings(&self) -> ArrayView2<Semitones>;

    fn num_intervals(&self) -> usize {
        self.intervals().len()
    }
    fn num_temperaments(&self) -> usize {
        self.temperaments().len()
    }
}

/// A stack of [Interval]s.
///
/// For every [ConcreteStackType] `t`, [Stack]s that reference `t` as their [stacktype][Stack::stacktype]
/// describe linear combinations o
/// f the base [intervals][ConcreteStackType::intervals] specified of `t`,
/// with adjustments due to the [temperaments][ConcreteStackType::temperaments] specified by `t`.
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
/// [correction_semitones][Stack::correction_semitones] if you're interested in the deviation from
/// the pure note due to temperaments.
///
/// * However, don't use a floating-point comparison with zero to figure out if a [Stack] contains
/// only pure intervals. Use [is_pure][Stack::is_pure] for that purpose.
///
/// Internally, the "temperament error" is tracked exactly (i.e. using only integer arithmetic).
/// This is what enables [is_pure][Stack::is_pure]. Even more importantly, we need that
/// representation for the "rollovers" that happen when a number of tempered intervals add up to
/// pure intervals. See the documentation comment of [increment][Stack::increment] for a discussion
/// of this phenomenon.
#[derive(Debug, PartialEq)]
pub struct Stack<T: StackType> {
    stacktype: Arc<T>,

    /// a `D`-vector of coefficients,  one for each base interval.
    coefficients: Vec<StackCoeff>,

    /// a `D x T`-matrix of "corrections": The colums correspond to temperaments, and the i-th
    /// entry of every column counts how many (fractions of) commas for the i-th interval from that
    /// temperament should be added to the stack.
    corrections: Array2<StackCoeff>,
}

// derive(Clone) doesn't treat cloning `Arc` correctly
impl<T: StackType> Clone for Stack<T> {
    fn clone(&self) -> Self {
        Stack {
            stacktype: self.stacktype.clone(),
            coefficients: self.coefficients.clone(),
            corrections: self.corrections.clone(),
        }
    }
}

/// A description which [Interval]s and [Temperament]s are to be used in a [Stack].
///
/// The numbers `D` of different intervals and `T` of temperaments are statically known.
#[derive(Debug, PartialEq)]
pub struct ConcreteStackType {
    /// A vector of the `D` intervals to be used
    intervals: Vec<Interval>,
    /// A vector of `T` temperaments, each for the `D` intervals
    temperaments: Vec<Temperament<StackCoeff>>,
    /// A `D x T` matrix of precomputed adjustments. Each column contains the (fractions) of commas
    /// for each interval (in the sense of `Temperament`s) of the corresponding temperament.
    precomputed_temperings: Array2<Semitones>,
}

impl StackType for ConcreteStackType {
    fn intervals(&self) -> &[Interval] {
        &self.intervals
    }

    fn temperaments(&self) -> &[Temperament<StackCoeff>] {
        &self.temperaments
    }

    fn precomputed_temperings(&self) -> ArrayView2<Semitones> {
        self.precomputed_temperings.view()
    }
}

impl ConcreteStackType {
    /// Construct a [ConcreteStackType] from [Interval][Interval]s and
    /// [Temperament][Temperament]s. The [dimension][Temperament::dimension] of the temperaments
    /// must be the number of intervals.
    pub fn new(intervals: Vec<Interval>, temperaments: Vec<Temperament<StackCoeff>>) -> Self {
        let precomputed_temperings =
            Array2::from_shape_fn((intervals.len(), temperaments.len()), |(i, t)| {
                let mut whole_comma = 0.0;
                for (i, c) in temperaments[t].comma(i).iter().enumerate() {
                    whole_comma += (*c as Semitones) * intervals[i].semitones;
                }
                whole_comma / temperaments[t].denominator(i) as Semitones
            });

        ConcreteStackType {
            intervals,
            temperaments,
            precomputed_temperings,
        }
    }
}

impl<T: StackType> Stack<T> {
    /// private: the absolute value of `corrections[[i,t]]` must be less than the
    /// [denominator][Temperament::denominator] of the `t`-th temperament for interval `i`.
    /// This invariant is enforced by all functions we expose.
    fn normalise(&mut self) {
        for (t, temper) in self.stacktype.temperaments().iter().enumerate() {
            for (i, _) in self.stacktype.intervals().iter().enumerate() {
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
    pub fn new(
        stacktype: Arc<T>,
        active_temperaments: &[bool],
        coefficients: Vec<StackCoeff>,
    ) -> Stack<T> {
        let corrections =
            Array2::from_shape_fn((coefficients.len(), active_temperaments.len()), |(i, t)| {
                if active_temperaments[t] {
                    coefficients[i]
                } else {
                    0
                }
            });

        let mut res = Stack {
            stacktype,
            coefficients,
            corrections,
        };
        res.normalise();

        res
    }

    pub fn stacktype(&self) -> Arc<T> {
        self.stacktype.clone()
    }

    pub fn coefficients(&self) -> &[StackCoeff] {
        &self.coefficients
    }

    pub fn semitones(&self) -> Semitones {
        let mut pure_semitones = 0.0;
        for (i, c) in self.coefficients.iter().enumerate() {
            pure_semitones += (*c as Semitones) * self.stacktype.intervals()[i].semitones;
        }
        pure_semitones + self.impure_semitones()
    }

    pub fn impure_semitones(&self) -> Semitones {
        let mut res = 0.0;
        for (ix, c) in self.corrections.indexed_iter() {
            res += (*c as Semitones) * self.stacktype.precomputed_temperings()[ix];
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
}

impl<T: StackType> ops::Add<&Stack<T>> for Stack<T> {
    type Output = Self;

    /// Addition of [Stack]s is a bit more involved than one might think at first glance: It
    /// involves more than adding corresponding [coefficients][Stack::coefficients], because of the
    /// effect of [temperaments][ConcreteStackType::temperaments].
    ///
    /// Read the documentation comment of [increment][Stack::increment], since addition is
    /// implemented following the same logic.
    ///
    /// Also, in many applications, [increment][Stack::increment]ing might be the cheaper option,
    /// because it doesn't require you to construct a second [Stack].
    fn add(mut self, x: &Self) -> Self {
        if !std::sync::Arc::ptr_eq(&self.stacktype, &x.stacktype) {
            panic!("tried to add two `Stack`s of different `StackType`s")
        }

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
pub mod stack_test_setup {
    use super::*;
    use ndarray::arr2;

    /// some base intervals: octaves, fifths, thirds.
    pub fn init_intervals() -> [Interval; 3] {
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
    }

    /// some example temperaments: quarter-comma meantone, and 12-EDO
    pub fn init_temperaments() -> [Temperament<StackCoeff>; 2] {
        [
            Temperament::new(
                "1/4-comma meantone".into(),
                arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]).view(),
            )
            .unwrap(),
            Temperament::new(
                "12edo".into(),
                arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]).view(),
            )
            .unwrap(),
        ]
    }

    /// an example [ConcreteStackType].
    pub fn init_stacktype() -> ConcreteStackType {
        ConcreteStackType::new(init_intervals().into(), init_temperaments().into())
    }
}

#[cfg(test)]
mod test {
    use super::stack_test_setup::*;
    use super::*;
    use approx::*;
    use ndarray::arr2;

    #[test]
    fn test_stack_semitones() {
        let st = Arc::new(init_stacktype());

        let octave = 12.0;
        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma = 3.0 * (80.0 / 81.0 as Semitones).log2();
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number. I don't care about
                                 // extreme numerical stbility.

        let s = Stack::new(st.clone(), &[false, true], vec![0, 0, 1]);
        assert_relative_eq!(s.impure_semitones(), edo12_third_error, max_relative = eps);
        assert_relative_eq!(s.semitones(), third + edo12_third_error, max_relative = eps);

        let s = Stack::new(st.clone(), &[true, false], vec![0, 4, 0]);
        assert_relative_eq!(s.impure_semitones(), 0.0, max_relative = eps);
        assert_relative_eq!(s.semitones(), third + 2.0 * octave, max_relative = eps);

        let s = Stack::new(st.clone(), &[true, false], vec![0, 6, 0]);
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

        let s = Stack::new(st.clone(), &[false, true], vec![0, 0, 7]);
        assert_relative_eq!(s.impure_semitones(), edo12_third_error, max_relative = eps);
        assert_relative_eq!(
            s.semitones(),
            7.0 * third + 7.0 * edo12_third_error,
            max_relative = eps
        );

        let s = Stack::new(st.clone(), &[true, true], vec![0, 5, 7]);
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
        let st = Arc::new(init_stacktype());

        let mut s = Stack::new(st.clone(), &[false, false], vec![0, 4, 3]);
        s = s + &Stack::new(st.clone(), &[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::new(st.clone(), &[true, false], vec![0, 4, 3]);
        s = s + &Stack::new(st.clone(), &[false, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [0, 0], [0, 0]]));

        let mut s = Stack::new(st.clone(), &[true, false], vec![0, 4, 3]);
        s = s + &Stack::new(st.clone(), &[true, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [2, 0], [0, 0]]));

        let mut s = Stack::new(st.clone(), &[true, true], vec![0, 4, 3]);
        s = s + &Stack::new(st.clone(), &[true, false], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![3 + 0, 0 + 2, 1 + 5]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [2, 4], [0, 0]]));

        let mut s = Stack::new(st.clone(), &[true, true], vec![0, 4, 3]);
        s = s + &Stack::new(st.clone(), &[true, true], vec![0, 2, 5]);
        assert_eq!(s.coefficients, vec![3 + 1, 0 + 2, 1 + 2]);
        assert_eq!(s.corrections, arr2(&[[0, 0], [2, 6], [0, 2]]));
    }
}
