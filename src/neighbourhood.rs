use std::sync::Arc;

use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
};

/// A description of the positions of representatives of all 12 pitch classes, relative to the
/// position of a reference note.
///
/// invariants:
/// - the [key_distance][Stack::key_distance] of the stack on index `ì` is `i`. In particular, the
/// first one (at index zero) must map to a unison on the keyboard.
/// - period_keys == period_keys.len()
#[derive(Debug, PartialEq)]
pub struct Neighbourhood<T: StackType> {
    pub stacks: Vec<Stack<T>>,
    pub period: Arc<Stack<T>>,
    pub period_keys: usize,
}

impl<T: StackType> Clone for Neighbourhood<T> {
    fn clone(&self) -> Self {
        Neighbourhood {
            stacks: self.stacks.clone(),
            period: self.period.clone(),
            period_keys: self.period_keys,
        }
    }
}

impl<T: FiveLimitStackType> Neighbourhood<T> {
    ///
    /// - `width` must be in `1..=12-index+offset`
    /// - `offset` must be in `0..width`
    /// - `index` must be in `0..=11`
    ///
    /// - `octave` must be a stack describing a pure octave
    pub fn fivelimit_new(
        stacktype: Arc<T>,
        octave: Arc<Stack<T>>,
        active_temperaments: &[bool],
        width: StackCoeff,
        index: StackCoeff,
        offset: StackCoeff,
    ) -> Self {
        let mut stacks = vec![Stack::new_zero(stacktype.clone()); 12];
        for i in (-index)..(12 - index) {
            let (octaves, fifths, thirds) = fivelimit_corridor(width, offset, i);
            let the_stack = &mut stacks[(7 * i).rem_euclid(12) as usize];
            the_stack.increment_at_index(&active_temperaments, stacktype.octave_index(), octaves);
            the_stack.increment_at_index(&active_temperaments, stacktype.fifth_index(), fifths);
            the_stack.increment_at_index(&active_temperaments, stacktype.third_index(), thirds);
        }
        Neighbourhood {
            stacks,
            period: octave,
            period_keys: 12,
        }
    }
}

impl<T: StackType> Neighbourhood<T> {
    /// the lowest and highest entry in the given dimension. The `axis` must be in the range
    /// `0..N`, where `N` is the [num_intervals][StackType::num_intervals].
    pub fn bounds(&self, axis: usize) -> (StackCoeff, StackCoeff) {
        // this initialisation is correct, because the first entry of `coefficients` is always a zero
        // vector
        let mut min = 0;
        let mut max = 0;
        for stack in &self.stacks {
            let curr = stack.coefficients()[axis];
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stack::stack_test_setup::init_fivelimit_stacktype;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_neighbours() {
        let st = Arc::new(init_fivelimit_stacktype());
        let period = Arc::new(Stack::new(st.clone(), &[false; 2], vec![1, 0, 0]));

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 12, 0, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-4, 7, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-5, 9, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 4, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-6, 11, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-3, 6, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-4, 8, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-5, 10, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 5, 0]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 3, 0, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -3, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -1, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 5, 0, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-3, 5, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 4, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 4, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 0, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 1, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 2, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 2, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 3, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 2, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 4, 0),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-2, 3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, 0, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 3, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 2, -1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 0, 1),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -1, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 0, 2),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![-1, 1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -1, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 1, 1]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );

        assert_eq!(
            Neighbourhood::fivelimit_new(st.clone(), period.clone(), &[false; 2], 4, 0, 3),
            Neighbourhood {
                stacks: vec![
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 0]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -3, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, -1, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![2, -3, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![0, 0, 2]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -1, 1]),
                    Stack::new(st.clone(), &vec![false; 2], vec![1, -2, 3]),
                    Stack::new(st.clone(), &vec![false; 2], vec![2, -3, 2]),
                ],
                period: period.clone(),
                period_keys: 12,
            }
        );
    }
}
