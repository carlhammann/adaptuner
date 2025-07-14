//! LU decomposition with minimal trait bounds

use std::ops::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};

use ndarray::{s, Array2, ArrayViewMut1, ArrayViewMut2, Zip};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{One, Signed, Zero};

#[derive(Debug)]
pub enum LUErr {
    MatrixNotSquare { nrows: usize, ncols: usize },
    WrongPermLenght { nrows: usize, perm_length: usize },
    MatrixDegenerate,
}

pub struct LU<'a, T> {
    a: ArrayViewMut2<'a, T>,
    perm: ArrayViewMut1<'a, usize>,
    n_swaps: usize,
}

/// Like [lu], only with a pre-set `better_pivot` argument that prefers simpler fractions
pub fn lu_rational<'a, T>(
    a: ArrayViewMut2<'a, Ratio<T>>,
    perm: ArrayViewMut1<'a, usize>,
) -> Result<LU<'a, Ratio<T>>, LUErr>
where
    T: Signed + Integer + RemAssign + DivAssign + MulAssign + SubAssign + AddAssign + Clone,
{
    let better_pivot = |a: &Ratio<T>, b: &Ratio<T>| {
        if a.is_zero() {
            return false;
        }
        if b.is_zero() {
            return true;
        }

        use std::cmp::Ordering::{Equal, Less};
        match (
            a.denom().abs().cmp(&b.denom().abs()),
            a.numer().abs().cmp(&b.numer().abs()),
        ) {
            (Less {}, Less {}) => return true,
            (Equal {}, Less {}) => return true,
            (Less {}, Equal {}) => return true,
            _ => return false,
        }
    };
    lu(a, perm, better_pivot)
}

/// input:
/// - `a` is a square matrix of dimension `n`
/// - `perm` has length `n+1`
/// - `better_pivot(a,b)` returns true iff `a` is a better pivot than `b`. The pivot is a number you want to divide by, so:
///   - Zero can never be a better pivot.
///   - For floating point numbers, bigger pivots (in absolute value) are better: dividing by a big number prevents blow-up.
///   - For rational numbers small denominators and numerators are preferrable.
///   - ...
///
/// output:
/// The content of the input arguments is overwritten with a compact representation of the LU
/// decomposition of `a`. The returned [LU] struct contains references to the same memory, and it
/// is advisable to only use the methods of that struct to access the contents.
pub fn lu<'a, T, P>(
    mut a: ArrayViewMut2<'a, T>,
    mut perm: ArrayViewMut1<'a, usize>,
    better_pivot: P,
) -> Result<LU<'a, T>, LUErr>
where
    T: Zero
        + for<'x> SubAssign<&'x T>
        + for<'x> MulAssign<&'x T>
        + for<'x> DivAssign<&'x T>
        + Clone,
    P: Fn(&T, &T) -> bool,
{
    let n = a.shape()[0];

    if !a.is_square() {
        let ncols = a.shape()[1];
        return Err(LUErr::MatrixNotSquare { nrows: n, ncols });
    }

    if n != perm.shape()[0] {
        return Err(LUErr::WrongPermLenght {
            nrows: n,
            perm_length: perm.shape()[0],
        });
    }

    for i in 0..n {
        perm[i] = i;
    }

    let mut n_swaps = 0;

    let mut pivot;
    let mut i_pivot;

    for i in 0..(n - 1) {
        // find the element on or below the diagonal with the biggest absolute value
        pivot = T::zero();
        i_pivot = i;
        for k in i..n {
            if better_pivot(&a[[k, i]], &pivot) {
                pivot.clone_from(&a[[k, i]]);
                i_pivot = k;
            }
        }

        if pivot.is_zero() {
            return Err(LUErr::MatrixDegenerate);
        }

        if i_pivot != i {
            // record the swap in `perm`
            let tmp_i = perm[i];
            perm[i] = perm[i_pivot];
            perm[i_pivot] = tmp_i;

            // count the number of swaps
            n_swaps += 1;

            // row interchange in a
            let (mut v, mut w) = a.multi_slice_mut((s![i, ..], s![i_pivot, ..]));
            Zip::from(&mut v).and(&mut w).for_each(std::mem::swap);
        }

        pivot.clone_from(&a[[i, i]]);
        for j in (i + 1)..n {
            a[[j, i]] /= &pivot;
            for k in (i + 1)..n {
                let mut tmp = a[[j, i]].clone();
                tmp *= &a[[i, k]];
                a[[j, k]] -= &tmp;
            }
        }
    }

    Ok(LU { a, perm, n_swaps })
}

impl<'a, T> LU<'a, T> {
    pub fn inverse(&self) -> Result<Array2<T>, LUErr>
    where
        T: Clone
            + for<'x> SubAssign<&'x T>
            + for<'x> MulAssign<&'x T>
            + for<'x> DivAssign<&'x T>
            + Zero
            + One,
    {
        let mut inv = Array2::zeros(self.a.raw_dim());
        self.inverse_inplace(&mut inv.view_mut())?;
        Ok(inv)
    }

    /// invariants: `inv` is at least as big as the original matrix. (Only the top left will be
    /// overwritten with the inverse if it is bigger.)
    pub fn inverse_inplace(&self, inv: &mut ArrayViewMut2<T>) -> Result<(), LUErr>
    where
        T: Clone
            + for<'x> SubAssign<&'x T>
            + for<'x> MulAssign<&'x T>
            + for<'x> DivAssign<&'x T>
            + Zero
            + One,
    {
        let n = self.a.shape()[0];
        if self.a[[n - 1, n - 1]].is_zero() {
            // This check suffices to ascertain that the determinant is nonzero; all other diagonal
            // elements of self.a are non-zero because [lu] would have thrown an error otherwise.
            return Err(LUErr::MatrixDegenerate);
        }
        let mut tmp = T::zero();
        for j in 0..n {
            for i in 0..n {
                inv[[i, j]] = if self.perm[i] == j {
                    T::one()
                } else {
                    T::zero()
                };

                for k in 0..i {
                    tmp.clone_from(&self.a[[i, k]]);
                    tmp *= &inv[[k, j]];
                    inv[[i, j]] -= &tmp;
                }
            }

            for i in (0..n).rev() {
                for k in (i + 1)..n {
                    tmp.clone_from(&self.a[[i, k]]);
                    tmp *= &inv[[k, j]];
                    inv[[i, j]] -= &tmp;
                }
                inv[[i, j]] /= &self.a[[i, i]];
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ndarray::{arr2, Array1, Array2};
    use num_rational::{BigRational, Ratio};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_big_inverse() {
        let mut a: Array2<BigRational> = arr2(&[
            [
                Ratio::new((-14441).into(), 14400.into()),
                Ratio::new(1.into(), 720.into()),
                Ratio::new(1.into(), 14400.into()),
                Ratio::new(1.into(), 721.into()),
            ],
            [
                Ratio::new(1.into(), 720.into()),
                Ratio::new((-73).into(), 720.into()),
                Ratio::new(1.into(), 20.into()),
                Ratio::new(1.into(), 20.into()),
            ],
            [
                Ratio::new(1.into(), 14400.into()),
                Ratio::new(1.into(), 20.into()),
                Ratio::new((-1441).into(), 14400.into()),
                Ratio::new(1.into(), 20.into()),
            ],
            [
                Ratio::new(1.into(), 720.into()),
                Ratio::new(1.into(), 20.into()),
                Ratio::new(1.into(), 20.into()),
                Ratio::new((-73).into(), 720.into()),
            ],
        ]);

        let expected: Array2<BigRational> = arr2(&[
            [
                Ratio::new("-519120".parse().unwrap(), "519121".parse().unwrap()),
                Ratio::new(
                    "-83518673040".parse().unwrap(),
                    "83574847153".parse().unwrap(),
                ),
                Ratio::new("-766221840".parse().unwrap(), "766741717".parse().unwrap()),
                Ratio::new(
                    "-83517609600".parse().unwrap(),
                    "83574847153".parse().unwrap(),
                ),
            ],
            [
                Ratio::new("-519120".parse().unwrap(), "519121".parse().unwrap()),
                Ratio::new(
                    "-4244737082400".parse().unwrap(),
                    "11939263879".parse().unwrap(),
                ),
                Ratio::new(
                    "-38554078320".parse().unwrap(),
                    "109534531".parse().unwrap(),
                ),
                Ratio::new(
                    "-4165872068160".parse().unwrap(),
                    "11939263879".parse().unwrap(),
                ),
            ],
            [
                Ratio::new("-519120".parse().unwrap(), "519121".parse().unwrap()),
                Ratio::new(
                    "-29416762250640".parse().unwrap(),
                    "83574847153".parse().unwrap(),
                ),
                Ratio::new(
                    "-277353890640".parse().unwrap(),
                    "766741717".parse().unwrap(),
                ),
                Ratio::new(
                    "-29416761187200".parse().unwrap(),
                    "83574847153".parse().unwrap(),
                ),
            ],
            [
                Ratio::new("-519120".parse().unwrap(), "519121".parse().unwrap()),
                Ratio::new(
                    "-4165872220080".parse().unwrap(),
                    "11939263879".parse().unwrap(),
                ),
                Ratio::new(
                    "-38554078320".parse().unwrap(),
                    "109534531".parse().unwrap(),
                ),
                Ratio::new(
                    "-4244736930480".parse().unwrap(),
                    "11939263879".parse().unwrap(),
                ),
            ],
        ]);

        let mut p = Array1::zeros(4);

        let lu = lu_rational(a.view_mut(), p.view_mut()).unwrap();

        let mut actual = Array2::zeros((4, 4));

        lu.inverse_inplace(&mut actual.view_mut()).unwrap();

        assert_eq!(actual, expected);
    }
}
