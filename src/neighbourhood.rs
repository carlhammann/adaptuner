//! A neighbourhood is a description of the tunings of some notes, relative to
//! a reference note.

use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    stack::{ScaledAdd, Stack},
    stacktype::r#trait::{OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType},
};

pub trait Neighbourhood<T: StackType> {
    /// Insert a tuning. If there's already a tuning for the (relative) note described by Stack,
    /// update. Returns a reference to the actually inserted Stack (which may be different in the
    /// case of [PeriodicNeighbourhood]s, where we store the representative in the "octave" above
    /// the reference)
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T>;

    /// Go through all stacks _that are actually stored_ (for example, in a
    /// [PeriodicNeighbourhood], only at most the entries for one peirod are stored) in the
    /// neighbourhood, with their offset to the reference.
    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F);

    /// like [Neighbourhood::for_each_stack], but allows mutation.
    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, f: F);

    /// Does this neighbourhood provide a tuning for a note with the given offset from the
    /// reference?
    ///
    /// Must return `true` for every `offset` in the case of [CompleteNeigbourhood]s.
    fn has_tuning_for(&self, offset: i8) -> bool;

    /// Like [Neighbourhood::try_get_relative_stack], but with an output argument `target`, which
    /// must remain unchanged if it returns `false`.
    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool;

    /// Return the Stack describing the interval with the given offset. Must return `Some` iff
    /// [Neighbourhood::has_tuning_for] returns true for the same offset.
    fn try_get_relative_stack(&self, offset: i8) -> Option<Stack<T>> {
        if self.has_tuning_for(offset) {
            let mut res = Stack::new_zero();
            let _ = self.try_write_relative_stack(&mut res, offset);
            Some(res)
        } else {
            None
        }
    }

    /// Must return `Some` for an implementation of [PeriodicNeighbourhood] to be valid.
    fn try_period(&self) -> Option<&Stack<T>>;

    /// Must return `Some` for an implementation of [AlignedPeriodicNeighbourhood] to be valid.
    fn try_period_index(&self) -> Option<usize>;

    /// Returns the lowest and highest entry in the given dimension. The `axis` must be in the
    /// range `0..N`, where `N` is the [StackType::num_intervals].
    fn bounds(&self, axis: usize) -> (StackCoeff, StackCoeff) {
        let (mut min, mut max) = (0, 0);
        self.for_each_stack(|_, stack| {
            let x = stack.target[axis];
            if x > max {
                max = x
            }
            if x < min {
                min = x
            }
        });
        (min, max)
    }

    fn name(&self) -> &str;

    fn set_name(&mut self, name: String);
}

/// Tunings for all notes, described by giving the tunings of all notes in the first "octave" above
/// the reference.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PeriodicComplete<T: StackType> {
    stacks: Vec<Stack<T>>,
    period: Stack<T>,
    name: String,
}

impl<T: StackType> PeriodicComplete<T> {
    /// invariants:
    /// - the [Stack::key_distance] of the stack on index `ì` of `stacks` is `i`. In particular,
    ///   the first one (at index zero) must map to a unison on the keyboard.
    /// - the length of `stacks` is the [Stack::key_distance] of the `period`.
    pub fn new(stacks: Vec<Stack<T>>, period: Stack<T>, name: String) -> Self {
        Self {
            stacks,
            period,
            name,
        }
    }
}

/// Like [PeriodicComplete], but with the invariant that the period is the one of the stack type
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicCompleteAligned<T: PeriodicStackType> {
    inner: PeriodicComplete<T>,
}

impl<T: PeriodicStackType> PeriodicCompleteAligned<T> {
    /// invariants like [PeriodicComplete::new()], only for the [PeriodicStackType::period] of `T`.
    pub fn new(stacks: Vec<Stack<T>>, name: String) -> Self {
        Self {
            inner: PeriodicComplete {
                stacks,
                period: Stack::from_pure_interval(T::period_index(), 1),
                name,
            },
        }
    }
}

impl<T: OctavePeriodicStackType> PeriodicCompleteAligned<T> {
    /// invariants like [PeriodicComplete::new()], only for the *actual* octave.
    pub fn from_octave_tunings(stacks: [Stack<T>; 12], name: String) -> Self {
        Self {
            inner: PeriodicComplete {
                stacks: stacks.into(),
                period: Stack::from_pure_interval(T::period_index(), 1),
                name,
            },
        }
    }
}

/// Marker trait of neighbourhoods that can return a note for every offset.
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
    fn period(&self) -> &Stack<T> {
        self.try_period()
            .expect("This should never happen: PeriodicNeighbourhood doesn't have a period")
    }

    /// Convenience: the [key_distance][Stack::key_distance] of the period.
    fn period_keys(&self) -> u8 {
        self.period().key_distance() as u8
    }
}

/// It's a logical error to implement this for non-[PeriodicStackType]s.
pub trait AlignedPeriodicNeighbourhood<T: StackType>: Neighbourhood<T> {
    fn period_index(&self) -> usize {
        self.try_period_index().expect(
            "This should never happen: AlignedPeriodicNeighbourhood doen't have a perios index.",
        )
    }
}

impl<T: StackType> Neighbourhood<T> for PeriodicComplete<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let n = self.period_keys() as StackCoeff;
        let quot = stack.key_distance().div_euclid(n);
        let rem = stack.key_distance().rem_euclid(n) as usize;
        self.stacks[rem].clone_from(stack);
        self.stacks[rem].scaled_add(-quot, &self.period);
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
        target.scaled_add(quot, &self.period);
        true
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        Some(&self.period)
    }

    fn try_period_index(&self) -> Option<usize> {
        None {}
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

impl<T: StackType> CompleteNeigbourhood<T> for PeriodicComplete<T> {}

impl<T: StackType> PeriodicNeighbourhood<T> for PeriodicComplete<T> {}

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

    fn try_period(&self) -> Option<&Stack<T>> {
        self.inner.try_period()
    }

    fn try_period_index(&self) -> Option<usize> {
        Some(T::period_index())
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn set_name(&mut self, name: String) {
        self.inner.set_name(name);
    }
}

impl<T: PeriodicStackType> CompleteNeigbourhood<T> for PeriodicCompleteAligned<T> {}

impl<T: PeriodicStackType> PeriodicNeighbourhood<T> for PeriodicCompleteAligned<T> {}

impl<T: PeriodicStackType> AlignedPeriodicNeighbourhood<T> for PeriodicCompleteAligned<T> {}
