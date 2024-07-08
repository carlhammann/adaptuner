use std::mem::MaybeUninit;

use crate::{
    interval::StackCoeff,
    util::dimension::{vector_from_elem, AtLeast, Bounded, Dimension, Vector},
};

/// A description of the positions of representatives of all 12 pitch classes, relative to the
/// position of a reference note.
///
/// invariants:
/// - the first entry of `coefficients` is a constant zero vector.
/// - the positions described by the other entries all correspond to intervals less than an octave.
#[derive(Debug, PartialEq, Clone)]
pub struct Neighbourhood<D: Dimension> {
    pub coefficients: [Vector<D, StackCoeff>; 12],
}

impl<D: Dimension + AtLeast<3> + Copy> Neighbourhood<D> {
    pub fn fivelimit_new(width: StackCoeff, index: StackCoeff, offset: StackCoeff) -> Self {
        let mut uninitialised: [MaybeUninit<Vector<D, StackCoeff>>; 12] =
            MaybeUninit::uninit_array();
        for i in 0..12 {
            uninitialised[i].write(vector_from_elem(0));
        }
        let mut coefficients = unsafe { MaybeUninit::array_assume_init(uninitialised) };
        fivelimit_neighbours(&mut coefficients, width, index, offset);
        Neighbourhood { coefficients }
    }

    pub fn fivelimit_udpate(&mut self, width: StackCoeff, index: StackCoeff, offset: StackCoeff) {
        fivelimit_neighbours(&mut self.coefficients, width, index, offset);
    }

    /// the lowest and highest entry in the given dimension
    pub fn bounds(&self, axis: Bounded<D>) -> (StackCoeff, StackCoeff) {
        // this initialisation is correct, because the first entry of `coefficients` is always a zero
        // vector
        let mut min = 0;
        let mut max = 0;
        for i in 1..12 {
            let curr = self.coefficients[i][axis];
            if curr < min {
                min = curr;
            }
            if curr > max {
                max = curr;
            }
        }
        (min, max)
    }
}

///
/// - `width` must be in `1..=12`
/// - `offset` must be in `0..width`
/// and the element at
///
fn fivelimit_corridor(
    width: StackCoeff,
    offset: StackCoeff,
    index: StackCoeff,
) -> (StackCoeff, StackCoeff, StackCoeff) {
    let (mut fifths, thirds) = fivelimit_corridor_no_offset(width, index + offset);
    fifths -= offset;
    let octaves = -(2 * thirds + 4 * fifths).div_euclid(7);
    (octaves, fifths, thirds)
}

fn fivelimit_corridor_no_offset(width: StackCoeff, index: StackCoeff) -> (StackCoeff, StackCoeff) {
    let thirds = index.div_euclid(width);
    let fifths = (width - 4) * thirds + index.rem_euclid(width);
    (fifths, thirds)
}

fn fivelimit_neighbours<D: Dimension + AtLeast<3>>(
    grid: &mut [Vector<D, StackCoeff>; 12],
    width: StackCoeff,  // 1..=12
    index: StackCoeff,  // 0..=11
    offset: StackCoeff, // 0..=(width-1)
) {
    for i in (-index)..(12 - index) {
        let (octaves, fifths, thirds) = fivelimit_corridor(width, offset, i);
        grid[(7 * i).rem_euclid(12) as usize][Bounded::new(0).unwrap()] = octaves;
        grid[(7 * i).rem_euclid(12) as usize][Bounded::new(1).unwrap()] = fifths;
        grid[(7 * i).rem_euclid(12) as usize][Bounded::new(2).unwrap()] = thirds;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::dimension::{fixed_sizes::Size3, vector};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_neighbours() {
        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(12, 0, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-4, 7, 0]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-5, 9, 0]).unwrap(),
                    vector(&[-2, 4, 0]).unwrap(),
                    vector(&[-6, 11, 0]).unwrap(),
                    vector(&[-3, 6, 0]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[-4, 8, 0]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[-5, 10, 0]).unwrap(),
                    vector(&[-2, 5, 0]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(3, 0, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[0, -1, 2]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[1, -3, 3]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[0, -1, 3]).unwrap(),
                    vector(&[1, -2, 2]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[1, -1, 1]).unwrap(),
                    vector(&[1, -2, 3]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(5, 0, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-3, 5, 1]).unwrap(),
                    vector(&[-2, 4, 0]).unwrap(),
                    vector(&[-2, 3, 2]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[-2, 4, 1]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[-1, 2, 2]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 0, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-1, 1, 2]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[-2, 3, 2]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[-1, 2, 2]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 1, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-1, 1, 2]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[-1, 3, -1]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[-1, 2, 2]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 2, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-1, 1, 2]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[-1, 3, -1]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[0, 2, -1]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 3, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[0, 1, -1]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[-1, 3, -1]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[0, 2, -1]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 4, 0),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[-2, 3, 1]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[0, 1, -1]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[-1, 3, -1]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[1, 0, -1]).unwrap(),
                    vector(&[-1, 3, 0]).unwrap(),
                    vector(&[0, 2, -1]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 0, 1),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[0, -1, 2]).unwrap(),
                    vector(&[-1, 2, 0]).unwrap(),
                    vector(&[-1, 1, 2]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[0, -1, 3]).unwrap(),
                    vector(&[-1, 2, 1]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[1, -1, 1]).unwrap(),
                    vector(&[-1, 2, 2]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 0, 2),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[0, -1, 2]).unwrap(),
                    vector(&[1, -2, 1]).unwrap(),
                    vector(&[-1, 1, 2]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[0, -1, 3]).unwrap(),
                    vector(&[1, -2, 2]).unwrap(),
                    vector(&[0, 1, 0]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[1, -1, 1]).unwrap(),
                    vector(&[1, -2, 3]).unwrap(),
                    vector(&[0, 1, 1]).unwrap(),
                ],
            }
        );

        assert_eq!(
            Neighbourhood::<Size3>::fivelimit_new(4, 0, 3),
            Neighbourhood {
                coefficients: [
                    vector(&[0, 0, 0]).unwrap(),
                    vector(&[0, -1, 2]).unwrap(),
                    vector(&[1, -2, 1]).unwrap(),
                    vector(&[1, -3, 3]).unwrap(),
                    vector(&[0, 0, 1]).unwrap(),
                    vector(&[0, -1, 3]).unwrap(),
                    vector(&[1, -2, 2]).unwrap(),
                    vector(&[2, -3, 1]).unwrap(),
                    vector(&[0, 0, 2]).unwrap(),
                    vector(&[1, -1, 1]).unwrap(),
                    vector(&[1, -2, 3]).unwrap(),
                    vector(&[2, -3, 2]).unwrap(),
                ],
            }
        );
    }
}
