//! A neighbourhood is a description of the tunings of some notes, relative to
//! a reference note.

use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    stack::{ScaledAdd, Stack},
    stacktype::r#trait::{IntervalBasis, OctavePeriodicIntervalBasis, StackCoeff},
};

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum SomeCompleteNeighbourhood<T: IntervalBasis> {
    PeriodicComplete(PeriodicComplete<T>),
}

impl<T: IntervalBasis> From<PeriodicComplete<T>> for SomeCompleteNeighbourhood<T> {
    fn from(pc: PeriodicComplete<T>) -> Self {
        Self::PeriodicComplete(pc)
    }
}

/// Tunings for all notes, described by giving the tunings of all notes in the first "octave-like"
/// period above the reference. The period can be an interval in the sense of
/// [IntervalBasis::try_period_index], but may also be a composite interval described by a [Stack]
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicComplete<T: IntervalBasis> {
    stacks: Vec<Stack<T>>,
    period: Stack<T>,
    name: String,
    period_index: Option<usize>,
}

impl<T: IntervalBasis> PeriodicComplete<T> {
    /// invariants:
    /// - the [Stack::key_distance] of the stack on index `ì` of `stacks` is `i`. In particular,
    ///   the first one (at index zero) must map to a unison on the keyboard.
    /// - the length of `stacks` is the [Stack::key_distance] of the `period`.
    pub fn new(stacks: Vec<Stack<T>>, period: Stack<T>, name: String) -> Self {
        Self {
            stacks,
            period,
            name,
            period_index: None {},
        }
    }
}

impl<T: OctavePeriodicIntervalBasis> PeriodicComplete<T> {
    /// invariants like [PeriodicComplete::new], only for the [PeriodicStackType::period] of `T`
    pub fn from_octave_tunings(name: String, stacks: [Stack<T>; 12]) -> Self {
        Self {
            stacks: stacks.into(),
            period: Stack::from_pure_interval(T::period_index(), 1),
            period_index: Some(T::period_index()),
            name,
        }
    }
}

// /// Like [PeriodicComplete], but some positions in the period may be undefined, i.e. have no tuning
// /// associated.
// pub struct PeriodicPartial<T: IntervalBasis> {
//     stacks: Vec<(Stack<T>, bool)>,
//     period: Stack<T>,
//     name: String,
//     period_index: Option<usize>,
// }

pub struct PartialNeighbourhood<T: IntervalBasis> {
    stacks: HashMap<i8, (Stack<T>, bool)>,
    name: String,
}

impl<T: IntervalBasis> PartialNeighbourhood<T> {
    pub fn new(name: String) -> Self {
        Self {
            stacks: HashMap::new(),
            name,
        }
    }

    pub fn clear_marks(&mut self) {
        self.stacks.values_mut().for_each(|(_, b)| *b = false);
    }

    pub fn mark<F: Fn(i8, &Stack<T>) -> bool>(&mut self, f: F) {
        for (i, (stack, mark)) in self.stacks.iter_mut() {
            *mark |= f(*i, stack);
        }
    }

    pub fn contains<F: Fn(&Stack<T>) -> bool>(&self, f: F) -> bool {
        self.stacks.values().any(|(s, _)| f(s))
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, i8, (Stack<T>, bool)> {
        self.stacks.iter()
    }
}

pub trait Neighbourhood<T: IntervalBasis> {
    /// Insert a tuning. If there's already a tuning for the (relative) note described by Stack,
    /// update. Returns a reference to the actually inserted Stack (which may be different in the
    /// case of [PeriodicNeighbourhood]s, where we store the representative in the "octave" above
    /// the reference)
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T>;

    /// Go through all stacks _that are actually stored_ (for example, in a
    /// [PeriodicNeighbourhood], only at most the entries for one period are stored) in the
    /// neighbourhood, with their offset to the reference.
    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F);

    fn for_each_stack_failing<E, F: FnMut(i8, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E>;

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

    /// If the period returned by [Neighbourhood::try_period] corresponds to a base interval,
    /// return that interval's index in the [IntervalBasis::intervals].
    fn try_period_index(&self) -> Option<usize>;

    /// Returns the lowest and highest entry in the given dimension. The `axis` must be in the
    /// range `0..N`, where `N` is the [IntervalBasis::num_intervals].
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

impl<T: IntervalBasis> Neighbourhood<T> for SomeCompleteNeighbourhood<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.insert(stack),
        }
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack(f),
        }
    }

    fn for_each_stack_failing<E, F: FnMut(i8, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_failing(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: i8) -> bool {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => {
                n.try_write_relative_stack(target, offset)
            }
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.try_period(),
        }
    }

    fn try_period_index(&self) -> Option<usize> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.try_period_index(),
        }
    }

    fn name(&self) -> &str {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.name(),
        }
    }

    fn set_name(&mut self, name: String) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.set_name(name),
        }
    }
}

impl<T: IntervalBasis> CompleteNeigbourhood<T> for SomeCompleteNeighbourhood<T> {}

impl<T: IntervalBasis> Neighbourhood<T> for PartialNeighbourhood<T> {
    /// If the [Stack::key_distance] of `stack` is not in the inclusive range from -128 to 127,
    /// this may misbehave! Only insert stacks whose key_distance could be an i8.
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let offset = stack.key_distance() as i8;
        if let Some((old_entry, _)) = self.stacks.get_mut(&offset) {
            old_entry.clone_from(stack);
        } else {
            self.stacks.insert(offset, (stack.clone(), false));
        }
        self.stacks.get(&offset).map(|(s, _)| s).unwrap()
    }

    fn for_each_stack<F: FnMut(i8, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, (stack, _)) in self.stacks.iter() {
            f(*i, stack);
        }
    }

    fn for_each_stack_failing<E, F: FnMut(i8, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, (stack, _)) in self.stacks.iter() {
            f(*i, stack)?;
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(i8, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, (stack, _)) in self.stacks.iter_mut() {
            f(*i, stack);
        }
    }

    fn has_tuning_for(&self, offset: i8) -> bool {
        self.stacks.contains_key(&offset)
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: i8) -> bool {
        if let Some((stack, _)) = self.stacks.get(&offset) {
            target.clone_from(stack);
            true
        } else {
            false
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        None {}
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

impl<T: IntervalBasis> Neighbourhood<T> for PeriodicComplete<T> {
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

    fn for_each_stack_failing<E, F: FnMut(i8, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as i8, stack)?;
        }
        Ok(())
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
        self.period_index
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

/// Marker trait of neighbourhoods that can return a note for every offset.
pub trait CompleteNeigbourhood<T: IntervalBasis>: Neighbourhood<T> {
    fn write_relative_stack(&self, target: &mut Stack<T>, offset: i8) {
        self.try_write_relative_stack(target, offset);
    }

    fn get_relative_stack(&self, offset: i8) -> Stack<T> {
        self.try_get_relative_stack(offset).expect(
            "This should never happen: CompleteNeigbourhood doesn't have a tuning for an offset!",
        )
    }
}

impl<T: IntervalBasis> CompleteNeigbourhood<T> for PeriodicComplete<T> {}

pub trait PeriodicNeighbourhood<T: IntervalBasis>: Neighbourhood<T> {
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

impl<T: IntervalBasis> PeriodicNeighbourhood<T> for PeriodicComplete<T> {}
