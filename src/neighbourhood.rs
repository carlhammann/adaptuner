use std::mem::MaybeUninit;

use crate::interval::stacktype::r#trait::StackCoeff;

/// A description of the positions of representatives of all 12 pitch classes, relative to the
/// position of a reference note.
///
/// invariants:
/// - the first entry of `coefficients` is a constant zero vector.
/// - the positions described by the other entries all correspond to intervals less than an octave.
#[derive(Debug, PartialEq, Clone)]
pub struct Neighbourhood {
    pub coefficients: [Vec<StackCoeff>; 12],
    pub width: StackCoeff,
    pub index: StackCoeff,
    pub offset: StackCoeff,
}

impl Neighbourhood {
    pub fn fivelimit_new(width: StackCoeff, index: StackCoeff, offset: StackCoeff) -> Self {
        let mut uninitialised: [MaybeUninit<Vec<StackCoeff>>; 12] = MaybeUninit::uninit_array();
        for i in 0..12 {
            uninitialised[i].write(vec![0, 0, 0]);
        }
        let mut coefficients = unsafe { MaybeUninit::array_assume_init(uninitialised) };
        fivelimit_neighbours(&mut coefficients, width, index, offset);
        Neighbourhood {
            coefficients,
            width,
            index,
            offset,
        }
    }

    pub fn fivelimit_udpate(&mut self, width: StackCoeff, index: StackCoeff, offset: StackCoeff) {
        fivelimit_neighbours(&mut self.coefficients, width, index, offset);
        self.width = width;
        self.index = index;
        self.offset = offset;
    }

    pub fn fivelimit_inc_width(&mut self) {
        if self.width < 12 - self.index + self.offset {
            self.fivelimit_udpate(self.width + 1, self.index, self.offset)
        }
    }

    pub fn fivelimit_dec_width(&mut self) {
        if self.width > 1 {
            self.fivelimit_udpate(self.width - 1, self.index, self.offset)
        }
        if self.offset >= self.width {
            self.fivelimit_udpate(self.width, self.index, self.width - 1);
        }
    }

    pub fn fivelimit_inc_offset(&mut self) {
        if self.offset < self.width - 1 {
            self.fivelimit_udpate(self.width, self.index, self.offset + 1)
        }
    }

    pub fn fivelimit_dec_offset(&mut self) {
        if self.offset > 0 {
            self.fivelimit_udpate(self.width, self.index, self.offset - 1)
        }
    }

    pub fn fivelimit_inc_index(&mut self) {
        if self.index < 11 {
            self.fivelimit_udpate(self.width, self.index + 1, self.offset)
        }
    }

    pub fn fivelimit_dec_index(&mut self) {
        if self.index > 0 {
            self.fivelimit_udpate(self.width, self.index - 1, self.offset)
        }
    }

    /// the lowest and highest entry in the given dimension
    pub fn bounds(&self, axis: usize) -> (StackCoeff, StackCoeff) {
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
/// - `width` must be in `1..=12-index+offset`
/// - `offset` must be in `0..width`
/// - `index` must be in `0..=11`
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

fn fivelimit_neighbours(
    grid: &mut [Vec<StackCoeff>; 12],
    width: StackCoeff,  // 1..=12
    index: StackCoeff,  // 0..=11
    offset: StackCoeff, // 0..=(width-1)
) {
    for i in (-index)..(12 - index) {
        let (octaves, fifths, thirds) = fivelimit_corridor(width, offset, i);
        grid[(7 * i).rem_euclid(12) as usize][0] = octaves;
        grid[(7 * i).rem_euclid(12) as usize][1] = fifths;
        grid[(7 * i).rem_euclid(12) as usize][2] = thirds;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_neighbours() {
        assert_eq!(
            Neighbourhood::fivelimit_new(12, 0, 0),
            Neighbourhood {
                width: 12,
                index: 0,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-4, 7, 0],
                    vec![-1, 2, 0],
                    vec![-5, 9, 0],
                    vec![-2, 4, 0],
                    vec![-6, 11, 0],
                    vec![-3, 6, 0],
                    vec![0, 1, 0],
                    vec![-4, 8, 0],
                    vec![-1, 3, 0],
                    vec![-5, 10, 0],
                    vec![-2, 5, 0],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(3, 0, 0),
            Neighbourhood {
                width: 3,
                index: 0,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![0, -1, 2],
                    vec![-1, 2, 0],
                    vec![1, -3, 3],
                    vec![0, 0, 1],
                    vec![0, -1, 3],
                    vec![1, -2, 2],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![1, -1, 1],
                    vec![1, -2, 3],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(5, 0, 0),
            Neighbourhood {
                width: 5,
                index: 0,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![-3, 5, 1],
                    vec![-2, 4, 0],
                    vec![-2, 3, 2],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![-2, 4, 1],
                    vec![-1, 3, 0],
                    vec![-1, 2, 2],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 0, 0),
            Neighbourhood {
                width: 4,
                index: 0,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![-1, 1, 2],
                    vec![0, 0, 1],
                    vec![-2, 3, 2],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![-1, 3, 0],
                    vec![-1, 2, 2],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 1, 0),
            Neighbourhood {
                width: 4,
                index: 1,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![-1, 1, 2],
                    vec![0, 0, 1],
                    vec![-1, 3, -1],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![-1, 3, 0],
                    vec![-1, 2, 2],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 2, 0),
            Neighbourhood {
                width: 4,
                index: 2,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![-1, 1, 2],
                    vec![0, 0, 1],
                    vec![-1, 3, -1],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![-1, 3, 0],
                    vec![0, 2, -1],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 3, 0),
            Neighbourhood {
                width: 4,
                index: 3,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![0, 1, -1],
                    vec![0, 0, 1],
                    vec![-1, 3, -1],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![-1, 3, 0],
                    vec![0, 2, -1],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 4, 0),
            Neighbourhood {
                width: 4,
                index: 4,
                offset: 0,
                coefficients: [
                    vec![0, 0, 0],
                    vec![-2, 3, 1],
                    vec![-1, 2, 0],
                    vec![0, 1, -1],
                    vec![0, 0, 1],
                    vec![-1, 3, -1],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![1, 0, -1],
                    vec![-1, 3, 0],
                    vec![0, 2, -1],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 0, 1),
            Neighbourhood {
                width: 4,
                index: 0,
                offset: 1,
                coefficients: [
                    vec![0, 0, 0],
                    vec![0, -1, 2],
                    vec![-1, 2, 0],
                    vec![-1, 1, 2],
                    vec![0, 0, 1],
                    vec![0, -1, 3],
                    vec![-1, 2, 1],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![1, -1, 1],
                    vec![-1, 2, 2],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 0, 2),
            Neighbourhood {
                width: 4,
                index: 0,
                offset: 2,
                coefficients: [
                    vec![0, 0, 0],
                    vec![0, -1, 2],
                    vec![1, -2, 1],
                    vec![-1, 1, 2],
                    vec![0, 0, 1],
                    vec![0, -1, 3],
                    vec![1, -2, 2],
                    vec![0, 1, 0],
                    vec![0, 0, 2],
                    vec![1, -1, 1],
                    vec![1, -2, 3],
                    vec![0, 1, 1],
                ],
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(4, 0, 3),
            Neighbourhood {
                width: 4,
                index: 0,
                offset: 3,
                coefficients: [
                    vec![0, 0, 0],
                    vec![0, -1, 2],
                    vec![1, -2, 1],
                    vec![1, -3, 3],
                    vec![0, 0, 1],
                    vec![0, -1, 3],
                    vec![1, -2, 2],
                    vec![2, -3, 1],
                    vec![0, 0, 2],
                    vec![1, -1, 1],
                    vec![1, -2, 3],
                    vec![2, -3, 2],
                ],
            }
        );
    }
}
