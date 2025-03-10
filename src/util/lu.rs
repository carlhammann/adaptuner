//! LU decomposition with minimal trait bounds

use std::ops::{DivAssign, Mul, MulAssign, SubAssign};

use ndarray::{s, ArrayView1, ArrayViewMut1, ArrayViewMut2, Zip};
use num_traits::{One, Signed, Zero};

#[derive(Debug)]
pub enum LUErr {
    MatrixNotSquare { nrows: usize, ncols: usize },
    WrongPermLenght { nrows: usize, perm_length: usize },
    MatrixDegenerate,
}

// This function changes `a`, so that it contain the matrices `L-E` and `U` s.t. `A=(L-E)+U` and `PA=LU`, where `P` is a permutation matrix that can be obtained from `perm`
//
// some encoding of the LU decomposition of
pub struct LU<'a, T> {
    a: ArrayViewMut2<'a, T>,
    perm: ArrayViewMut1<'a, usize>,
}

/// invariants:
/// - `a` is a square matrix of dimension `n`
/// - `perm` has length `n+1`
///
/// output:
/// The content of the input arguments is overwritten with a compact representation of the LU
/// decomposition of `a`. The returned [LU] struct contains references to the same memory, and it
/// is advisable to only use the methods of that struct to access the contents.
pub fn lu<'a, T>(
    mut a: ArrayViewMut2<'a, T>,
    mut perm: ArrayViewMut1<'a, usize>,
) -> Result<LU<'a, T>, LUErr>
where
    T: Zero
        + Signed
        + PartialOrd
        + for<'x> SubAssign<&'x T>
        + for<'x> MulAssign<&'x T>
        + for<'x> DivAssign<&'x T>
        + Clone,
{
    let n = a.shape()[0];

    if !a.is_square() {
        let ncols = a.shape()[1];
        return Err(LUErr::MatrixNotSquare { nrows: n, ncols });
    }

    if n + 1 != perm.shape()[0] {
        return Err(LUErr::WrongPermLenght {
            nrows: n,
            perm_length: perm.shape()[0],
        });
    }

    for i in 0..n {
        perm[i] = i;
    }

    let mut pivot;
    let mut i_pivot;

    for i in 0..(n - 1) {
        // find the element on or below the diagonal with the biggest absolute value
        pivot = T::zero();
        i_pivot = i;
        for k in i..n {
            let tmp = a[[k, i]].abs();
            if tmp > pivot {
                pivot = tmp;
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
            perm[n] += 1;

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

    Ok(LU { a, perm })
}

impl<'a, T> LU<'a, T> {
    /// invariants: `inv` is at least as big as the original matrix. (Only the top left will be
    /// overwritten with the inverse if it is bigger.)
    pub fn inverse_inplace(&self, inv: &mut ArrayViewMut2<T>)
    where
        T: Clone
            + for<'x> SubAssign<&'x T>
            + for<'x> MulAssign<&'x T>
            + for<'x> DivAssign<&'x T>
            + Zero
            + One,
    {
        let n = self.a.shape()[0];
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

        let mut p = Array1::zeros(5);

        let lu = lu(a.view_mut(), p.view_mut()).unwrap();

        let mut actual = Array2::zeros((4, 4));

        lu.inverse_inplace(&mut actual.view_mut());

        assert_eq!(actual, expected);
    }
}
