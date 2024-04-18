//! Treating the idea of "tempering intervals" in the abstract setting. A [Temperament] is
//! basically an explanation of how stacks of tempered intervals relate to stacks of pure
//! intervals, where "stacks" are integer linear combinations.

use ndarray::{Array1, Array2};
use num_integer::Integer;
use num_traits::Signed;
use std::{error::Error, fmt, ops};

use fractionfree;

/// A description of a temperament, i.e. "how much you detune" some intervals.
///
/// Assume we're working in a setting with `D` base intervals (octaves, fifths, thirds,
/// sevenths...) which we conceive of as "pure". Sometimes, we want to describe a slightly detuned
/// version of this set of intervals. How much we detune the intervals, in terms of fractions of
/// linear combinations of the base intervals, is what an element of this type encodes.
#[derive(Debug)]
pub struct Temperament<const D: usize, I> {
    pub name: Box<str>,
    commas: [[I; D]; D],
    denominators: [I; D],
}

impl<const D: usize, I> Temperament<D, I>
where
    I: Copy
        + ops::Div<Output = I>
        + ops::DivAssign
        + ops::AddAssign
        + ops::SubAssign
        + Signed
        + Integer
        + 'static,
{
    /// The error "tempered out" by the `i`-th interval, given as (the coefficients of) a linear
    /// combination of pure intervals.
    ///
    /// This may not be the actual adjustment that has to be applied to an individual interval; in
    /// order obtain that, divide by the `i`-th [denominator][Temperament::denominator].
    ///
    /// The following invariants always hold:
    ///
    /// * `gcd(x.denominator(i), x.comma(i)[0], ..., x.comma(i)[D]) == 1`
    ///
    /// * `x.denominator(i) > 0`
    pub fn comma(&self, i: usize) -> &[I; D] {
        &self.commas[i]
    }

    /// See the documentation of [comma][Temperament::comma].
    pub fn denominator(&self, i: usize) -> I {
        self.denominators[i]
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
    /// # use adaptuner::interval::*;
    /// # fn main () -> Result<(),TemperamentErr> {
    /// let tempered = [[0, 4, 0], [1, 0, 0], [0, 0, 1]];
    /// let pure     = [[2, 0, 1], [1, 0, 0], [0, 0, 1]];
    ///
    /// let t = Temperament::new("name of temperament".into(), tempered, pure)?;
    ///
    /// assert_eq!(t.comma(0), &[0, 0, 0]);
    /// assert_eq!(t.comma(1), &[2, -4, 1]);
    /// assert_eq!(t.comma(2), &[0, 0, 0]);
    /// assert_eq!(t.denominator(0), 1);
    /// assert_eq!(t.denominator(1), 4);
    /// assert_eq!(t.denominator(2), 1);
    /// # Ok(())
    /// # }
    ///```
    /// The first rows of `tempered` and `pure` encode the constraint that four tempered fifths should
    /// be equal to two pure octaves plus one pure third. The other two rows rows say that tempered
    /// octaves and thirds should be equal to their pure counterparts. Thus, the temperament described
    /// by `tempered` and `pure` is: "Make four fifths the same size as two octaves and a third, and
    /// don't detune octaves and thirds". This is, of course, the definition of quarter-comma meantone.
    ///
    /// The output confirms this: We see that the only non-zero [comma][Temperament::comma] is the
    /// one corresponding to the second base interval (the fifths), and that the error that is
    /// tempered is "2 octaves - 4 fifts + 1 third" (which is exactly the definition of a syntonic
    /// comma downwards). The corresponding [denominator][Temperament::denominator] says that this
    /// error is distributed between four fifths.
    pub fn new(
        name: Box<str>,
        mut tempered: [[I; D]; D],
        mut pure: [[I; D]; D],
    ) -> Result<Temperament<D, I>, TemperamentErr> {
        let a = Array2::from_shape_fn((D, D), |(i, j)| tempered[i][j]);
        let b = Array2::from_shape_fn((D, D), |(i, j)| pure[i][j]);

        let tempered_lu = match fractionfree::lu(a) {
            Err(fractionfree::LinalgErr::LURankDeficient) => {
                return Err(TemperamentErr::Indeterminate)
            }
            Err(e) => return Err(TemperamentErr::FromLinalgErr(e)),
            Ok(x) => x,
        };

        let (det, adj) = tempered_lu.inverse()?;

        let mut e = adj.dot(&b);
        for i in 0..D {
            e[[i, i]] -= det;
        }

        let mut k = Array1::from_elem(D, det);
        fractionfree::normalise(&mut k.view_mut(), &mut e.view_mut())?;

        // overwrite `tempered` and the first row of `pure` with the new values:
        for i in 0..D {
            pure[0][i] = k[i];
            for j in 0..D {
                tempered[i][j] = e[[i, j]];
            }
        }

        Ok(Temperament {
            name,
            commas: tempered,
            denominators: pure[0],
        })
    }
}

#[derive(Debug)]
pub enum TemperamentErr {
    FromLinalgErr(fractionfree::LinalgErr),
    Indeterminate,
}

impl fmt::Display for TemperamentErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemperamentErr::FromLinalgErr(_) => write!(f, "integer linear algebra error"),
            TemperamentErr::Indeterminate => write!(
                f,
                "constraints on tempered and pure intervals are indeterminate"
            ),
        }
    }
}

impl Error for TemperamentErr {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TemperamentErr::FromLinalgErr(e) => Some(e),
            _ => None,
        }
    }
}

impl From<fractionfree::LinalgErr> for TemperamentErr {
    fn from(value: fractionfree::LinalgErr) -> Self {
        Self::FromLinalgErr(value)
    }
}
