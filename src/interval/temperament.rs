//! Treating the idea of "tempering intervals" in the abstract setting. A [Temperament] is a
//! specification of how stacks of tempered intervals relate to stacks of pure intervals.

use std::ops::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};

use ndarray::{s, Array1, Array2, ArrayView1, ArrayView2, ArrayViewMut1};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{One, Signed};

use crate::util::lu::{lu_rational, LUErr};

/// A description of a temperament, i.e. "how much you detune" some intervals.
///
/// Assume we're working in a setting with `D` base intervals (octaves, fifths, thirds,
/// sevenths...) which we conceive of as "pure". Sometimes, we want to describe a slightly detuned
/// version of this set of intervals. How much we detune the intervals, in terms of rational linear
/// combinations of the base intervals, is what an element of this type encodes.
#[derive(Debug, Clone)]
pub struct Temperament<I> {
    pub name: String,

    /// a `D x D` matrix. The i-th row describes the "comma" by which the i-th interval is
    /// detuned. The comma is given as a rational combination of base intervals, and the
    /// coefficients of that linear combination are what the row contains.
    adjustments: Array2<Ratio<I>>,
}

impl<I> Temperament<I>
where
    I: Signed
        + Integer
        + RemAssign
        + DivAssign
        + MulAssign
        + SubAssign
        + AddAssign
        + Copy
        + One
        + 'static,
{
    /// The error "tempered out" by the `i`-th interval, given as (the coefficients of) a rational
    /// combination of pure intervals.
    pub fn comma(&self, i: usize) -> ArrayView1<Ratio<I>> {
        self.adjustments.slice(s![i, ..])
    }

    /// The adjustment applied by the temperament to a stack of pure invervals with the given
    /// coefficients.
    pub fn adjustment(&self, coefficients: ArrayView1<I>) -> Array1<Ratio<I>> {
        let mut output = Array1::zeros(coefficients.raw_dim());
        self.add_adjustment(coefficients, output.view_mut());
        output
    }

    /// Like [Self::adjustment], only with an output argument that will be mutated. The adjustment
    /// will be added to whatever is already in `output`.
    pub fn add_adjustment(&self, coefficients: ArrayView1<I>, mut output: ArrayViewMut1<Ratio<I>>) {
        let d = coefficients.len();
        for i in 0..d {
            for j in 0..d {
                output[i] += &self.adjustments[[j, i]] * coefficients[j];
            }
        }
    }

    /// Compute the [Temperament] of `D` intervals from `D` pairwise identifications of notes.
    ///
    /// A geometric intuition might help. If there are `D` base intervals, we've got two
    /// `D`-dimensional grids: The grid of pure intervals, and the grid of tempered intervals. In order
    /// to define the tempered intervals, we'll have to specify for `D` points of the "tempered grid"
    /// where they should end up on the "pure grid".
    //
    /// The arguments are two square matrices of the same size:
    ///
    /// * Each row of `tempered` describes an integer linear combination of tempered intervals. This
    /// matrix must be invertible.
    ///
    /// * Each row of `pure` describes an integer linear combination of pure intervals.
    ///
    /// Let's make an example. Assume that we've got three base intervals: octaves, fifths, and thirds.
    /// Consider the following:
    /// ```
    /// # use ndarray::{arr1, arr2};
    /// # use adaptuner::interval::temperament::*;
    /// # use num_rational::Ratio;
    /// # fn main () {
    /// let tempered = arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]);
    /// let pure     = arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]);
    ///
    /// let t = Temperament::new(String::from("name of temperament"), tempered.view(), pure.view()).unwrap();
    ///
    /// assert_eq!(t.adjustment(arr1(&[1, 0, 0]).view()),
    ///            arr1(&[Ratio::from_integer(0), Ratio::from_integer(0), Ratio::from_integer(0)]));
    /// assert_eq!(t.adjustment(arr1(&[0, 1, 0]).view()),
    ///            arr1(&[Ratio::new(2, 4), Ratio::new(-4, 4), Ratio::new(1, 4)]));
    /// assert_eq!(t.adjustment(arr1(&[0, 0, 1]).view()),
    ///            arr1(&[Ratio::from_integer(0), Ratio::from_integer(0), Ratio::from_integer(0)]));
    /// # }
    ///```
    /// The first rows of `tempered` and `pure` encode the constraint that four tempered fifths should
    /// be equal to two pure octaves plus one pure third. The other two rows rows say that tempered
    /// octaves and thirds should be equal to their pure counterparts. Thus, the temperament described
    /// by `tempered` and `pure` is: "Make four fifths the same size as two octaves and a third, and
    /// don't detune octaves and thirds". This is, of course, the definition of quarter-comma meantone.
    ///
    /// The output confirms this: We see that the only non-zero
    /// [adjustment][Temperament::adjustment] is the one corresponding to the second base interval
    /// (the fifths), and that the error that is tempered is "1/4 of (2 octaves - 4 fifts + 1
    /// third)" (which is exactly the definition of a quarter syntonic comma downwards).
    pub fn new(
        name: String,
        tempered: ArrayView2<I>,
        pure: ArrayView2<I>,
    ) -> Result<Temperament<I>, TemperamentErr> {
        let mut tmp = Array2::from_shape_fn(tempered.raw_dim(), |(i, j)| {
            Ratio::from_integer(tempered[[i, j]])
        });

        let mut tempered_lu_perm = Array1::zeros(tempered.shape()[0]);
        let tempered_lu = match lu_rational(tmp.view_mut(), tempered_lu_perm.view_mut()) {
            Err(LUErr::MatrixDegenerate) => return Err(TemperamentErr::Indeterminate),
            Err(e) => return Err(TemperamentErr::FromLinalgErr(e)),
            Ok(x) => x,
        };

        // initialisation of tempered_inv doesn't matter.
        let mut tempered_inv = Array2::zeros(tempered.raw_dim());
        tempered_lu.inverse_inplace(&mut tempered_inv.view_mut())?;

        tmp = Array2::from_shape_fn(tempered.raw_dim(), |(i, j)| {
            Ratio::from_integer(pure[[i, j]])
        });
        let mut adjustments = tempered_inv.dot(&tmp);
        adjustments.diag_mut().map_mut(|x| {
            *x -= Ratio::from_integer(I::one());
        });

        Ok(Temperament { name, adjustments })
    }
}

#[derive(Debug)]
pub enum TemperamentErr {
    FromLinalgErr(LUErr),
    Indeterminate,
}

impl From<LUErr> for TemperamentErr {
    fn from(value: LUErr) -> Self {
        Self::FromLinalgErr(value)
    }
}
