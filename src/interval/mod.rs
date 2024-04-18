//! Everything that has to do with (stacks of) intervals, pure or tempered.

use std::ops;

mod temperament;
pub use temperament::*;

/// The type of integer coefficients used in [Stack]s
pub type StackCoeff = i32;

/// The type of interval sizes measured in equally tempered semitones
pub type Semitones = f64;

/// A "base" interval.
#[derive(Debug)]
pub struct Interval {
    /// the human-facing name of the interval.
    pub name: Box<str>,
    /// the size of the interval in semitones. This is a logarithmic measure: "size in cents
    /// divided by 100".
    pub semitones: Semitones,
}

/// A description which [Interval]s and [Temperament]s are to be used in a [Stack].
///
/// The numbers `D` of different intervals and `T` of temperaments are statically known.
#[derive(Debug)]
pub struct StackType<const D: usize, const T: usize> {
    intervals: [Interval; D],
    temperaments: [Temperament<D, StackCoeff>; T],
    precomputed_temperings: [[Semitones; T]; D],
}

impl<const D: usize, const T: usize> StackType<D, T> {
    /// The base intervals to be used by [Stack]s of this type.
    pub fn intervals(&self) -> &[Interval; D] {
        &self.intervals
    }

    /// The [Temperament]s that may be used by [Stack]s of this type.
    pub fn temperaments(&self) -> &[Temperament<D, StackCoeff>; T] {
        &self.temperaments
    }

    /// Construct a [StackType] from its [intervals][StackType::intervals] and
    /// [temperaments][StackType::temperaments].
    pub fn new(intervals: [Interval; D], temperaments: [Temperament<D, StackCoeff>; T]) -> Self {
        let mut precomputed_temperings = [[0.0; T]; D];

        for i in 0..D {
            for t in 0..T {
                precomputed_temperings[i][t] =
                    pure_stack_semitones(temperaments[t].comma(i), &intervals)
                        / temperaments[t].denominator(i) as Semitones;
            }
        }

        StackType {
            intervals,
            temperaments,
            precomputed_temperings,
        }
    }
}

/// A stack of [Interval]s.
///
/// For every [StackType] `t`, [Stack]s that reference `t` as their [stacktype][Stack::stacktype]
/// describe linear combinations of the base [intervals][StackType::intervals] specified of `t`,
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
#[derive(Clone, Debug)]
pub struct Stack<'a, const D: usize, const T: usize> {
    stacktype: &'a StackType<D, T>,
    coefficients: [StackCoeff; D],
    corrections: [[StackCoeff; T]; D],
}

impl<'a, const D: usize, const T: usize> Stack<'a, D, T> {
    /// See the documentation comment of [Stack].
    pub fn stacktype(&self) -> &'a StackType<D, T> {
        self.stacktype
    }

    /// See the documentation comment of [Stack].
    pub fn coefficients(&self) -> &[StackCoeff; D] {
        &self.coefficients
    }

    /// See the documentation comment of [Stack].
    pub fn semitones(&self) -> Semitones {
        pure_stack_semitones(&self.coefficients, &self.stacktype.intervals)
            + self.correction_semitones()
    }

    /// See the documentation comment of [Stack].
    pub fn correction_semitones(&self) -> Semitones {
        let mut res = 0.0;

        for i in 0..D {
            for j in 0..T {
                res += self.corrections[i][j] as Semitones
                    * self.stacktype.precomputed_temperings[i][j];
            }
        }

        res
    }

    /// See the documentation comment of [Stack].
    pub fn is_pure(&self) -> bool {
        self.corrections.iter().fold(true, |accouter, cs| {
            cs.iter().fold(true, |accinner, c| *c == 0 && accinner) && accouter
        })
    }

    /// private: the absolute value of `corrections[i][t]` must be less than the
    /// [denominator][Temperament::denominator] of the `t`-th temperament for interval `i`.
    /// This invariant is enforced by all functions we expose.
    fn normalise(&mut self) {
        for t in 0..T {
            for i in 0..D {
                let comma = self.stacktype.temperaments[t].comma(i);
                let denominator = &self.stacktype.temperaments[t].denominator(i);

                let quot = self.corrections[i][t] / denominator;
                let rem = self.corrections[i][t] % denominator;

                for j in 0..D {
                    self.coefficients[j] += quot * comma[j]
                }

                self.corrections[i][t] = rem;
            }
        }
    }

    /// Add some more intervals to a stack.
    ///
    /// Assume the [Stack] to be modified has [stacktype][Stack::stacktype] `t`. Then,
    ///
    /// * the `coefficients` tell how many of each of the base [intervals][StackType::intervals]
    /// specified by `t` should be added.
    ///
    /// * `active_temperaments[i]` indicates whether the added intervals should be tweaked by the
    /// `i`-th of the [temperaments][StackType::temperaments] of the `t`. (That is, if the
    /// `active_temperaments` are the constant `false` vector, the added intervals will be pure,
    /// otherwise the selected [Temperament]s will apply.)
    ///
    /// Since the goal of tempering is to make stacks of slightly detuned intervals "fit into"
    /// stacks of pure intervals, this function tracks "rollovers", when a number of tempered intervals
    /// add up to yield pure intervals.
    ///
    /// Using trusty old quarter-comma meantone as an example, assume that that the initial stack
    /// of intervals contains three fifths and one third. (i.e. relative to C, that would describe
    /// a C# two octaves higher). Let's also assume that the fifts are each a quarter comma flat,
    /// and that the third is pure. Then [increment][Stack::increment]ing by another quarter-comma
    /// fifth will bring us to G#, obtained as "C plus four quarter-comma fifths plus one pure
    /// third", which, by definition of the quarter-comma fifths is exactly "C plus two octaves
    /// plus two pure thirds".
    ///
    /// Scenarios like this are handled correctly by this function: Whenever enough "temperament
    /// error" has accumulated to reach a pure interval, the [coefficients][Stack::coefficients] of
    /// the [increment][Stack::increment]ed stack will reflect the pure interval, and its
    /// internally stored representation of the temperament errors will be reset accordingly.
    pub fn increment(&mut self, active_temperaments: &[bool; T], coefficients: &[StackCoeff; D]) {
        for i in 0..D {
            self.coefficients[i] += coefficients[i];
        }

        for t in 0..T {
            if active_temperaments[t] {
                for i in 0..D {
                    self.corrections[i][t] += coefficients[i];
                }
            }
        }
        self.normalise();
    }

    /// Build a stack for a given [StackType]. The logic around `active_temperaments` and
    /// `coefficients` is the same as for [increment][Stack::increment].
    pub fn new(
        stacktype: &'a StackType<D, T>,
        active_temperaments: &[bool; T],
        coefficients: [StackCoeff; D],
    ) -> Self {
        let mut corrections = [[0; T]; D];
        for t in 0..T {
            if active_temperaments[t] {
                for i in 0..D {
                    corrections[i][t] = coefficients[i];
                }
            }
        }

        let mut res = Stack {
            stacktype,
            coefficients,
            corrections,
        };
        res.normalise();

        res
    }
}

/// private: compute the size of the composite interval described by a linear combination of
/// [Interval]s (without any temperament).
fn pure_stack_semitones<const D: usize>(
    coefficients: &[StackCoeff; D],
    intervals: &[Interval; D],
) -> Semitones {
    let mut sum = 0.0;
    for i in 0..D {
        sum = sum + (coefficients[i] as Semitones) * intervals[i].semitones;
    }
    sum
}

impl<'a, const D: usize, const T: usize> ops::Add<&Stack<'a, D, T>> for Stack<'a, D, T> {
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
    fn add(mut self, x: &Self) -> Self {
        if !std::ptr::eq(self.stacktype, x.stacktype) {
            panic!("tried to add two `Stack`s of different `StackType`s")
        }

        for i in 0..D {
            self.coefficients[i] += x.coefficients[i];
            for t in 0..T {
                self.corrections[i][t] += x.corrections[i][t];
            }
        }
        self.normalise();

        self
    }
}

#[cfg(test)]
pub mod stack_test_setup {
    use super::*;

    /// some base intervals: octaves, fifths, thirds.
    pub fn init_intervals() -> [Interval; 3] {
        [
            Interval {
                name: "octave".into(),
                semitones: 12.0,
            },
            Interval {
                name: "fifth".into(),
                semitones: 12.0 * (3.0 / 2.0 as Semitones).log2(),
            },
            Interval {
                name: "third".into(),
                semitones: 12.0 * (5.0 / 4.0 as Semitones).log2(),
            },
        ]
    }

    /// some example temperaments: quarter-comma meantone, and 12-EDO
    pub fn init_temperaments() -> [Temperament<3, StackCoeff>; 2] {
        [
            Temperament::new(
                "1/4-comma meantone".into(),
                [[0, 4, 0], [1, 0, 0], [0, 0, 1]],
                [[2, 0, 1], [1, 0, 0], [0, 0, 1]],
            )
            .unwrap(),
            Temperament::new(
                "12edo".into(),
                [[0, 12, 0], [0, 0, 3], [1, 0, 0]],
                [[7, 0, 0], [1, 0, 0], [1, 0, 0]],
            )
            .unwrap(),
        ]
    }

    /// an example [StackType].
    pub fn init_stacktype() -> StackType<3, 2> {
        StackType::new(init_intervals(), init_temperaments())
    }
}

#[cfg(test)]
mod test {
    use super::{stack_test_setup::init_stacktype, *};
    use approx::*;

    #[test]
    fn test_stack_semitones() {
        let st = init_stacktype();

        let octave = 12.0;
        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma = 3.0 * (80.0 / 81.0 as Semitones).log2();
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number. I don't care about
                                 // extreme numerical stbility.

        let s = Stack::new(&st, &[false, true], [0, 0, 1]);
        assert_relative_eq!(
            s.correction_semitones(),
            edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(s.semitones(), third + edo12_third_error, max_relative = eps);

        let s = Stack::new(&st, &[true, false], [0, 4, 0]);
        assert_relative_eq!(s.correction_semitones(), 0.0, max_relative = eps);
        assert_relative_eq!(s.semitones(), third + 2.0 * octave, max_relative = eps);

        let s = Stack::new(&st, &[true, false], [0, 6, 0]);
        assert_relative_eq!(
            s.correction_semitones(),
            2.0 * quarter_comma,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones(),
            2.0 * octave + third + 2.0 * fifth + 2.0 * quarter_comma,
            max_relative = eps
        );

        let s = Stack::new(&st, &[false, true], [0, 0, 7]);
        assert_relative_eq!(
            s.correction_semitones(),
            edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones(),
            7.0 * third + 7.0 * edo12_third_error,
            max_relative = eps
        );

        let s = Stack::new(&st, &[true, true], [0, 5, 7]);
        assert_relative_eq!(
            s.correction_semitones(),
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
        let st = init_stacktype();

        let s1 = Stack::new(&st, &[false, false], [0, 4, 3]);
        let s2 = Stack::new(&st, &[false, false], [0, 2, 5]);
        let s = s1 + &s2;
        assert_eq!(s.coefficients, [0 + 0, 4 + 2, 3 + 5]);
        assert_eq!(s.corrections, [[0, 0], [0, 0], [0, 0]]);

        let s1 = Stack::new(&st, &[true, false], [0, 4, 3]);
        let s2 = Stack::new(&st, &[false, false], [0, 2, 5]);
        let s = s1 + &s2;
        assert_eq!(s.coefficients, [2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, [[0, 0], [0, 0], [0, 0]]);

        let s1 = Stack::new(&st, &[true, false], [0, 4, 3]);
        let s2 = Stack::new(&st, &[true, false], [0, 2, 5]);
        let s = s1 + &s2;
        assert_eq!(s.coefficients, [2 + 0, 0 + 2, 4 + 5]);
        assert_eq!(s.corrections, [[0, 0], [2, 0], [0, 0]]);

        let s1 = Stack::new(&st, &[true, true], [0, 4, 3]);
        let s2 = Stack::new(&st, &[true, false], [0, 2, 5]);
        let s = s1 + &s2;
        assert_eq!(s.coefficients, [3 + 0, 0 + 2, 1 + 5]);
        assert_eq!(s.corrections, [[0, 0], [2, 4], [0, 0]]);

        let s1 = Stack::new(&st, &[true, true], [0, 4, 3]);
        let s2 = Stack::new(&st, &[true, true], [0, 2, 5]);
        let s = s1 + &s2;
        assert_eq!(s.coefficients, [3 + 1, 0 + 2, 1 + 2]);
        assert_eq!(s.corrections, [[0, 0], [2, 6], [0, 2]]);
    }
}
