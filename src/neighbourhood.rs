use std::{collections::HashMap, sync::Arc};

use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
};

/// A description of the positions of representatives of consecutive pitch classes, relative to the
/// position of a reference note.
///
/// invariants:
/// - the [key_distance][Stack::key_distance] of the stack on index `ì` is `i`. In particular, the
/// first one (at index zero) must map to a unison on the keyboard.
/// - period_keys == period_keys.len()
#[derive(Debug, PartialEq)]
pub enum Neighbourhood<T: StackType> {
    PeriodicComplete {
        stacks: Vec<Stack<T>>,
        period: Arc<Stack<T>>,
        period_keys: usize,
    },
    PeriodicPartial {
        /// invariant: the keys are all in the range 0..=(period_keys-1)
        stacks: HashMap<usize, Stack<T>>,
        period: Arc<Stack<T>>,
        period_keys: usize,
    },
}

impl<T: StackType> Neighbourhood<T> {
    pub fn period(&self) -> Arc<Stack<T>> {
        match self {
            Neighbourhood::PeriodicComplete { period, .. } => period.clone(),
            Neighbourhood::PeriodicPartial { period, .. } => period.clone(),
        }
    }
    pub fn period_keys(&self) -> usize {
        match self {
            Neighbourhood::PeriodicComplete { period_keys, .. } => *period_keys,
            Neighbourhood::PeriodicPartial { period_keys, .. } => *period_keys,
        }
    }

    /// insert a stack into a neighbourhood. If there's already a stack for the pitch class
    /// (remember, this is modulo period_keys), replace it. This function will also normalise the
    /// provided stack before storing, (i.e. subtract as many [period]s as necessary, in order to
    /// make it smaller than the [period_keys].)
    pub fn insert(&mut self, mut stack: Stack<T>) -> Stack<T> {
        let height = stack.key_distance();
        let quot = height.div_euclid(self.period_keys() as StackCoeff);
        let rem = height.rem_euclid(self.period_keys() as StackCoeff) as usize; // Yes, these casts are correct... height may be negative!
        stack.add_mul(-quot, &self.period());

        match self {
            Neighbourhood::PeriodicComplete { stacks, .. } => {
                stacks[rem].clone_from(&stack);
                stack
            }
            Neighbourhood::PeriodicPartial { stacks, .. } => {
                let _ = stacks.insert(rem, stack.clone());
                stack
            }
        }
    }

    pub fn relative_stack_for_key_offset(&self, offset: i8) -> Option<Stack<T>> {
        let rem = offset.rem_euclid(self.period_keys() as i8) as usize;
        let quot = offset.div_euclid(self.period_keys() as i8) as StackCoeff;
        let mut the_stack = Stack::new_zero(self.period().stacktype());
        the_stack.add_mul(quot, &self.period());
        match self {
            Neighbourhood::PeriodicComplete { stacks, .. } => {
                the_stack.add_mul(1, &stacks[rem as usize]);
                Some(the_stack)
            }
            Neighbourhood::PeriodicPartial { stacks, .. } => match stacks.get(&rem) {
                None => None,
                Some(increment) => {
                    the_stack.add_mul(1, increment);
                    Some(the_stack)
                }
            },
        }
    }

    pub fn for_each_stack<F: FnMut(usize, &Stack<T>) -> ()>(&self, mut f: F) {
        match self {
            Neighbourhood::PeriodicComplete { stacks, .. } => {
                for (i, s) in stacks.iter().enumerate() {
                    f(i, s);
                }
            }
            Neighbourhood::PeriodicPartial { stacks, .. } => {
                for (&i, s) in stacks.iter() {
                    f(i, s);
                }
            }
        }
    }

    pub fn for_each_stack_mut<F: FnMut(usize, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        match self {
            Neighbourhood::PeriodicComplete { stacks, .. } => {
                for (i, s) in stacks.iter_mut().enumerate() {
                    f(i, s);
                }
            }
            Neighbourhood::PeriodicPartial { stacks, .. } => {
                for (&i, s) in stacks.iter_mut() {
                    f(i, s);
                }
            }
        }
    }
}

impl<T: StackType> Clone for Neighbourhood<T> {
    fn clone(&self) -> Self {
        match self {
            Neighbourhood::PeriodicComplete {
                stacks,
                period,
                period_keys,
            } => Neighbourhood::PeriodicComplete {
                stacks: stacks.clone(),
                period: period.clone(),
                period_keys: *period_keys,
            },
            Neighbourhood::PeriodicPartial {
                stacks,
                period,
                period_keys,
            } => Neighbourhood::PeriodicPartial {
                stacks: stacks.clone(),
                period: period.clone(),
                period_keys: *period_keys,
            },
        }
    }
}

impl<T: FiveLimitStackType> Neighbourhood<T> {
    /// Generate a complete set of 12 notes, with a sensible five-limit tuning. TODO: explain the
    /// arguments, if we decide to keep these functions.
    ///
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
        Neighbourhood::PeriodicComplete {
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
        match self {
            Neighbourhood::PeriodicComplete { stacks, .. } => {
                for stack in stacks {
                    let curr = stack.coefficients()[axis];
                    if curr < min {
                        min = curr;
                    }
                    if curr > max {
                        max = curr;
                    }
                }
            }
            Neighbourhood::PeriodicPartial { stacks, .. } => {
                for (_, stack) in stacks {
                    let curr = stack.coefficients()[axis];
                    if curr < min {
                        min = curr;
                    }
                    if curr > max {
                        max = curr;
                    }
                }
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
            Neighbourhood::PeriodicComplete {
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
