//! A neighbourhood is a description of the tunings of some notes, relative to
//! a reference note.

use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{
        FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType,
    },
};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub enum SomeNeighbourhood<T: StackType> {
    PeriodicComplete(PeriodicComplete<T>),
    PeriodicPartial(PeriodicPartial<T>),
    // This one is excluded, as it only exists for [PeriodicStackType]s
    // PeriodicCompleteAligned(PeriodicCompleteAligned<T>),
}

/// Tunings for all notes, described by giving the tunings of all notes in the first "octave" above
/// the reference.
///
/// invariants:
/// - the [key_distance][Stack::key_distance] of the stack on index `ì` is `i`. In particular, the
/// first one (at index zero) must map to a unison on the keyboard.
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicComplete<T: StackType> {
    pub stacks: Vec<Stack<T>>,
    pub period: Stack<T>,
}

/// Like [PeriodicComplete], but with the invariant that the period is the one of the stack type
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicCompleteAligned<T: PeriodicStackType> {
    pub inner: PeriodicComplete<T>,
}

/// Tunings for some notes and their "octave equivalents" described by giving tunings of some notes
/// in the first "octave" over the reference.
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicPartial<T: StackType> {
    /// invariant: the keys are all in the range 0..=(period_keys-1)
    pub stacks: HashMap<usize, Stack<T>>,
    pub period: Stack<T>,
}

pub trait Neighbourhood<T: StackType> {
    /// Insert a tuning. If there's already a tuning for the (relative) note described by Stack,
    /// update. Returns a reference to the  actually inserted Stack (which may be different in the case of
    /// periodic neighbourhoods, where we store the representative in the "octave" above the
    /// reference)
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T>;

    /// Go through all stacks _that are actually stored_ (for example, in a periodic neighbourhood,
    /// only at most the entries for one peirod are stored) in the neighbourhood, with their offset
    /// to the reference.
    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F);

    /// like [for_each_stack], but allows mutation.
    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, f: F);

    /// Does this neighbourhood provide a tuning for a note with the given offset from the
    /// reference?
    fn has_tuning_for(&self, offset: i8) -> bool;

    /// must return true iff has_tuning_for returns true for the same offset. Must leave `target`
    /// unchanged if it returns `false`
    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool;

    /// must return Some iff has_tuning_for returns true for the same offset.
    fn try_get_relative_stack(&self, offset: i8) -> Option<Stack<T>> {
        if self.has_tuning_for(offset) {
            let mut res = Stack::new_zero();
            let _ = self.try_write_relative_stack(&mut res, offset);
            Some(res)
        } else {
            None
        }
    }

    /// the lowest and highest entry in the given dimension. The `axis` must be in the range
    /// `0..N`, where `N` is the [num_intervals][StackType::num_intervals].
    fn bounds(&self, axis: usize) -> (StackCoeff, StackCoeff) {
        let (mut min, mut max) = (0, 0);
        self.for_each_stack(|_, stack| {
            let x = stack.coefficients()[axis];
            if x > max {
                max = x
            }
            if x < min {
                min = x
            }
        });
        (min, max)
    }
}

/// Marker trait of neighbourhoods that can return a Note for every offset.
pub trait CompleteNeigbourhood<T: StackType>: Neighbourhood<T> {
    fn write_relative_stack(&self, target: &mut Stack<T>, offset: i8) {
        self.try_write_relative_stack(target, offset);
    }

    fn get_relative_stack(&self, offset: i8) -> Stack<T> {
        self.try_get_relative_stack(offset).expect(
            "This should never happen: CompleteNeigbourhood doesn't have a tuning for an offset!",
        )
    }
}

pub trait PeriodicNeighbourhood<T: StackType>: Neighbourhood<T> {
    /// The "octave": keys will be tuned relative to the highest note that can be obtained by
    /// shifting the reference a number (negative, zero, or positive) of these periods.
    fn period(&self) -> &Stack<T>;

    /// Convenience: the [key_distance][Stack::key_distance] of the period.
    fn period_keys(&self) -> u8 {
        self.period().key_distance() as u8
    }
}

/// Marker trait for periodic neighbourhoods whose period is the one of the stack type.
pub trait AlignedPeriodicNeighbourhood<T: PeriodicStackType>: PeriodicNeighbourhood<T> {}

impl<T: StackType> Neighbourhood<T> for PeriodicComplete<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let n = self.period_keys() as StackCoeff;
        let quot = stack.key_distance().div_euclid(n);
        let rem = stack.key_distance().rem_euclid(n) as usize;
        self.stacks[rem].clone_from(stack);
        self.stacks[rem].add_mul(-quot, &self.period);
        &self.stacks[rem]
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as i8, stack)
        }
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, stack) in self.stacks.iter_mut().enumerate() {
            f(i as i8, stack)
        }
    }

    fn has_tuning_for(&self, _: i8) -> bool {
        true
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        let n = self.period_keys() as i8;
        let quot = offset.div_euclid(n) as StackCoeff;
        let rem = offset.rem_euclid(n) as usize;
        target.clone_from(&self.stacks[rem]);
        target.add_mul(quot, &self.period);
        true
    }
}

impl<T: StackType> CompleteNeigbourhood<T> for PeriodicComplete<T> {}

impl<T: StackType> PeriodicNeighbourhood<T> for PeriodicComplete<T> {
    fn period(&self) -> &Stack<T> {
        &self.period
    }
}

impl<T: StackType> Neighbourhood<T> for PeriodicPartial<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let n = self.period_keys() as StackCoeff;
        let quot = stack.key_distance().div_euclid(n);
        let rem = stack.key_distance().rem_euclid(n) as usize;
        match self.stacks.get_mut(&rem) {
            None => {
                let mut the_stack = stack.clone();
                the_stack.add_mul(-quot, &self.period);
                self.stacks.insert(rem, the_stack);
            }
            Some(target) => {
                target.clone_from(stack);
                target.add_mul(-quot, &self.period);
            }
        }
        self.stacks.get(&rem).expect("this can't happen: No entry for Stack just inserted into PeriodicPartial neighbourhood")
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, mut f: F) {
        for (&i, stack) in self.stacks.iter() {
            f(i as i8, stack)
        }
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (&i, stack) in self.stacks.iter_mut() {
            f(i as i8, stack)
        }
    }

    fn has_tuning_for(&self, offset: i8) -> bool {
        let n = self.period_keys() as i8;
        let rem = offset.rem_euclid(n) as usize;
        self.stacks.contains_key(&rem)
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        let n = self.period_keys() as i8;
        let quot = offset.div_euclid(n) as StackCoeff;
        let rem = offset.rem_euclid(n) as usize;
        match self.stacks.get(&rem) {
            None => false,
            Some(stack) => {
                target.clone_from(stack);
                target.add_mul(quot, &self.period);
                true
            }
        }
    }
}

impl<T: StackType> PeriodicNeighbourhood<T> for PeriodicPartial<T> {
    fn period(&self) -> &Stack<T> {
        &self.period
    }
}

impl<T: StackType> Neighbourhood<T> for SomeNeighbourhood<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        match self {
            SomeNeighbourhood::PeriodicComplete(n) => n.insert(stack),
            SomeNeighbourhood::PeriodicPartial(n) => n.insert(stack),
        }
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(n) => n.for_each_stack(f),
            SomeNeighbourhood::PeriodicPartial(n) => n.for_each_stack(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(n) => n.for_each_stack_mut(f),
            SomeNeighbourhood::PeriodicPartial(n) => n.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: i8) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(n) => n.has_tuning_for(offset),
            SomeNeighbourhood::PeriodicPartial(n) => n.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(n) => n.try_write_relative_stack(target, offset),
            SomeNeighbourhood::PeriodicPartial(n) => n.try_write_relative_stack(target, offset),
        }
    }
}

impl<T: PeriodicStackType> Neighbourhood<T> for PeriodicCompleteAligned<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        self.inner.insert(stack)
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F) {
        self.inner.for_each_stack(f);
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, f: F) {
        self.inner.for_each_stack_mut(f);
    }

    fn has_tuning_for(&self, offset: i8) -> bool {
        self.inner.has_tuning_for(offset)
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        self.inner.try_write_relative_stack(target, offset)
    }
}
impl<T: PeriodicStackType> CompleteNeigbourhood<T> for PeriodicCompleteAligned<T> {}
impl<T: PeriodicStackType> PeriodicNeighbourhood<T> for PeriodicCompleteAligned<T> {
    fn period(&self) -> &Stack<T> {
        self.inner.period()
    }
}
impl<T: PeriodicStackType> AlignedPeriodicNeighbourhood<T> for PeriodicCompleteAligned<T> {}

/// Generate a complete set of 12 notes, with a sensible five-limit tuning. TODO: explain the
/// arguments, if we decide to keep these functions.
///
///
/// - `width` must be in `1..=12-index+offset`
/// - `offset` must be in `0..width`
/// - `index` must be in `0..=11`
pub fn new_fivelimit_neighbourhood<T: FiveLimitStackType + OctavePeriodicStackType>(
    active_temperaments: &[bool],
    width: StackCoeff,
    index: StackCoeff,
    offset: StackCoeff,
) -> PeriodicCompleteAligned<T> {
    let mut stacks = vec![Stack::new_zero(); 12];
    for i in (-index)..(12 - index) {
        let (octaves, fifths, thirds) = fivelimit_corridor(width, offset, i);
        let the_stack = &mut stacks[(7 * i).rem_euclid(12) as usize];
        the_stack.increment_at_index(&active_temperaments, T::octave_index(), octaves);
        the_stack.increment_at_index(&active_temperaments, T::fifth_index(), fifths);
        the_stack.increment_at_index(&active_temperaments, T::third_index(), thirds);
    }
    PeriodicCompleteAligned {
        inner: PeriodicComplete {
            stacks,
            period: Stack::from_pure_interval(T::period_index()),
        },
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
    use pretty_assertions::assert_eq;

    type MockStackType = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

    #[test]
    fn test_neighbours() {
        let period = Stack::<MockStackType>::new(&[false; 2], vec![1, 0, 0]);

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 12, 0, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-4, 7, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-5, 9, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 4, 0]),
                        Stack::new(&vec![false; 2], vec![-6, 11, 0]),
                        Stack::new(&vec![false; 2], vec![-3, 6, 0]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![-4, 8, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![-5, 10, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 5, 0]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 3, 0, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![0, -1, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![1, -3, 3]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![0, -1, 3]),
                        Stack::new(&vec![false; 2], vec![1, -2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![1, -1, 1]),
                        Stack::new(&vec![false; 2], vec![1, -2, 3]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 5, 0, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-3, 5, 1]),
                        Stack::new(&vec![false; 2], vec![-2, 4, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 4, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 0, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 1, 2]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 1, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 1, 2]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, -1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 2, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 1, 2]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, -1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![0, 2, -1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 3, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![0, 1, -1]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, -1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![0, 2, -1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 4, 0),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![-2, 3, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![0, 1, -1]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, -1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![1, 0, -1]),
                        Stack::new(&vec![false; 2], vec![-1, 3, 0]),
                        Stack::new(&vec![false; 2], vec![0, 2, -1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 0, 1),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![0, -1, 2]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 0]),
                        Stack::new(&vec![false; 2], vec![-1, 1, 2]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![0, -1, 3]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 1]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![1, -1, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 0, 2),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![0, -1, 2]),
                        Stack::new(&vec![false; 2], vec![1, -2, 1]),
                        Stack::new(&vec![false; 2], vec![-1, 1, 2]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![0, -1, 3]),
                        Stack::new(&vec![false; 2], vec![1, -2, 2]),
                        Stack::new(&vec![false; 2], vec![0, 1, 0]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![1, -1, 1]),
                        Stack::new(&vec![false; 2], vec![1, -2, 3]),
                        Stack::new(&vec![false; 2], vec![0, 1, 1]),
                    ],
                    period: period.clone(),
                }
            }
        );

        assert_eq!(
            new_fivelimit_neighbourhood(&[false; 2], 4, 0, 3),
            PeriodicCompleteAligned {
                inner: PeriodicComplete {
                    stacks: vec![
                        Stack::new(&vec![false; 2], vec![0, 0, 0]),
                        Stack::new(&vec![false; 2], vec![0, -1, 2]),
                        Stack::new(&vec![false; 2], vec![1, -2, 1]),
                        Stack::new(&vec![false; 2], vec![1, -3, 3]),
                        Stack::new(&vec![false; 2], vec![0, 0, 1]),
                        Stack::new(&vec![false; 2], vec![0, -1, 3]),
                        Stack::new(&vec![false; 2], vec![1, -2, 2]),
                        Stack::new(&vec![false; 2], vec![2, -3, 1]),
                        Stack::new(&vec![false; 2], vec![0, 0, 2]),
                        Stack::new(&vec![false; 2], vec![1, -1, 1]),
                        Stack::new(&vec![false; 2], vec![1, -2, 3]),
                        Stack::new(&vec![false; 2], vec![2, -3, 2]),
                    ],
                    period: period.clone(),
                }
            }
        );
    }
}
