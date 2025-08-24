//! A neighbourhood is a description of the tunings of some notes, relative to
//! a reference note.

use std::collections::BTreeMap;

use ndarray::ArrayView1;
use num_rational::Ratio;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    stack::{key_distance_from_coefficients, ScaledAdd, Stack},
    stacktype::r#trait::{IntervalBasis, PeriodicIntervalBasis, StackCoeff},
};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum SomeNeighbourhood<T: IntervalBasis> {
    PeriodicComplete(PeriodicComplete<T>),
    PeriodicPartial(PeriodicPartial<T>),
    Partial(Partial<T>),
}

impl<T: IntervalBasis> SomeNeighbourhood<T> {
    /// returns true iff all entries were cleared. This can only happen for partial neighbourhoods
    pub fn clear(&mut self) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(_) => return false,
            SomeNeighbourhood::PeriodicPartial(n) => n.clear(),
            SomeNeighbourhood::Partial(n) => n.clear(),
        }
        true
    }
}

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
    pub stacks: Vec<Stack<T>>,
    pub period: Stack<T>,
    pub period_index: Option<usize>,
}

impl<T: PeriodicIntervalBasis> PeriodicComplete<T> {
    /// invariants:
    /// - the [Stack::key_distance] of the stack on index `ì` of `stacks` is `i`.
    /// - the length of `stacks` is the [IntervalBasis::period_index] of `T`, if it exists
    pub fn new_periodic(stacks: Vec<Stack<T>>) -> Self {
        Self {
            stacks,
            period: Stack::from_pure_interval(T::period_index(), 1),
            period_index: Some(T::period_index()),
        }
    }
}

/// Like [PeriodicComplete], but some positions in the period may be undefined, i.e. have no tuning
/// associated.
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicPartial<T: IntervalBasis> {
    pub stacks: Vec<(Stack<T>, bool)>,
    pub period: Stack<T>,
    pub period_index: Option<usize>,
}

impl<T: IntervalBasis> PeriodicPartial<T> {
    pub fn new_from_period_index(period_index: usize) -> Self {
        Self {
            stacks: vec![
                (Stack::new_zero(), false);
                T::intervals()[period_index].key_distance as usize
            ],
            period: Stack::from_pure_interval(period_index, 1),
            period_index: Some(period_index),
        }
    }

    pub fn clear(&mut self) {
        self.stacks.iter_mut().for_each(|(_, b)| *b = false);
    }
}

impl<T: IntervalBasis> PeriodicNeighbourhood<T> for PeriodicPartial<T> {}

impl<T: IntervalBasis> Neighbourhood<T> for PeriodicPartial<T> {
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T> {
        let n = self.period_keys();
        let d = key_distance_from_coefficients::<T>(target);
        let quot = d.div_euclid(n);
        let rem = d.rem_euclid(n) as usize;
        self.stacks[rem].0.target.assign(&target);
        self.stacks[rem].0.actual.assign(&actual);
        self.stacks[rem].0.scaled_add(-quot, &self.period);
        self.stacks[rem].1 = true;
        &self.stacks[rem].0
    }

    fn insert_zero(&mut self) {
        self.stacks[0].0.target.fill(0);
        self.stacks[0].0.actual.fill(0.into());
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, (stack, valid)) in self.stacks.iter().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)
            }
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, (stack, valid)) in self.stacks.iter().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)?;
            }
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, (stack, valid)) in self.stacks.iter_mut().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)
            }
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let rem = offset.rem_euclid(n) as usize;
        self.stacks[rem].1
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let quot = offset.div_euclid(n);
        let rem = offset.rem_euclid(n) as usize;
        if self.stacks[rem].1 {
            target.clone_from(&self.stacks[rem].0);
            target.scaled_add(quot, &self.period);
            true
        } else {
            false
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        Some(&self.period)
    }

    fn try_period_index(&self) -> Option<usize> {
        self.period_index
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Partial<T: IntervalBasis> {
    pub stacks: BTreeMap<StackCoeff, Stack<T>>,
}

impl<T: IntervalBasis> Partial<T> {
    pub fn new() -> Self {
        Self {
            stacks: BTreeMap::new(),
        }
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, StackCoeff, Stack<T>> {
        self.stacks.iter()
    }

    pub fn clear(&mut self) {
        self.stacks.clear()
    }
}

pub trait Neighbourhood<T: IntervalBasis> {
    /// Insert a tuning. If there's already a tuning for the (relative) note described by Stack,
    /// update. Returns a reference to the actually inserted Stack (which may be different in the
    /// case of [PeriodicNeighbourhood]s, where we store the representative in the "octave" above
    /// the reference)
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        self.insert_target_actual(stack.target.view(), stack.actual.view())
    }

    /// Inserts the zero stack for offset zero
    fn insert_zero(&mut self);

    /// Like [Self::insert], but with the [Stack::target] and [Stack::actual] as separate
    /// arguments.
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T>;

    /// Go through all stacks _that are actually stored_ (for example, in a
    /// [PeriodicNeighbourhood], only at most the entries for one period are stored) in the
    /// neighbourhood, with their offset to the reference.
    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F);

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E>;

    /// like [Neighbourhood::for_each_stack], but allows mutation.
    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F);

    /// Does this neighbourhood provide a tuning for a note with the given offset from the
    /// reference?
    ///
    /// Must return `true` for every `offset` in the case of [CompleteNeigbourhood]s.
    fn has_tuning_for(&self, offset: StackCoeff) -> bool;

    /// Like [Neighbourhood::try_get_relative_stack], but with an output argument `target`, which
    /// must remain unchanged if it returns `false`.
    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool;

    /// Return the Stack describing the interval with the given offset. Must return `Some` iff
    /// [Neighbourhood::has_tuning_for] returns true for the same offset.
    fn try_get_relative_stack(&self, offset: StackCoeff) -> Option<Stack<T>> {
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
}

impl<T: IntervalBasis> Neighbourhood<T> for SomeNeighbourhood<T> {
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.insert_target_actual(target, actual),
            SomeNeighbourhood::PeriodicPartial(x) => x.insert_target_actual(target, actual),
            SomeNeighbourhood::Partial(x) => x.insert_target_actual(target, actual),
        }
    }

    fn insert_zero(&mut self) {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.insert_zero(),
            SomeNeighbourhood::PeriodicPartial(x) => x.insert_zero(),
            SomeNeighbourhood::Partial(x) => x.insert_zero(),
        }
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack(f),
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack_failing(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack_failing(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack_failing(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack_mut(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack_mut(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.has_tuning_for(offset),
            SomeNeighbourhood::PeriodicPartial(x) => x.has_tuning_for(offset),
            SomeNeighbourhood::Partial(x) => x.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_write_relative_stack(target, offset),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_write_relative_stack(target, offset),
            SomeNeighbourhood::Partial(x) => x.try_write_relative_stack(target, offset),
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_period(),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_period(),
            SomeNeighbourhood::Partial(x) => x.try_period(),
        }
    }

    fn try_period_index(&self) -> Option<usize> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_period_index(),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_period_index(),
            SomeNeighbourhood::Partial(x) => x.try_period_index(),
        }
    }
}

impl<T: IntervalBasis> Neighbourhood<T> for SomeCompleteNeighbourhood<T> {
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => {
                n.insert_target_actual(target, actual)
            }
        }
    }

    fn insert_zero(&mut self) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(x) => x.insert_zero(),
        }
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack(f),
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_failing(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
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
}

impl<T: IntervalBasis> CompleteNeigbourhood<T> for SomeCompleteNeighbourhood<T> {}

impl<T: IntervalBasis> Neighbourhood<T> for Partial<T> {
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T> {
        let offset = key_distance_from_coefficients::<T>(target);
        if let Some(old_entry) = self.stacks.get_mut(&offset) {
            old_entry.target.assign(&target);
            old_entry.actual.assign(&actual);
        } else {
            self.stacks.insert(
                offset,
                Stack::from_target_and_actual(target.to_owned(), actual.to_owned()),
            );
        }
        self.stacks.get(&offset).unwrap()
    }

    fn insert_zero(&mut self) {
        if let Some(old_entry) = self.stacks.get_mut(&0) {
            old_entry.target.fill(0);
            old_entry.actual.fill(0.into());
        } else {
            self.stacks.insert(0, Stack::new_zero());
        }
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, stack) in self.stacks.iter() {
            f(*i, stack);
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, stack) in self.stacks.iter() {
            f(*i, stack)?;
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, stack) in self.stacks.iter_mut() {
            f(*i, stack);
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        self.stacks.contains_key(&offset)
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        if let Some(stack) = self.stacks.get(&offset) {
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
}

impl<T: IntervalBasis> Neighbourhood<T> for PeriodicComplete<T> {
    fn insert_target_actual(
        &mut self,
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
    ) -> &Stack<T> {
        let n = self.period_keys();
        let d = key_distance_from_coefficients::<T>(target);
        let quot = d.div_euclid(n);
        let rem = d.rem_euclid(n) as usize;
        self.stacks[rem].target.assign(&target);
        self.stacks[rem].actual.assign(&actual);
        self.stacks[rem].scaled_add(-quot, &self.period);
        &self.stacks[rem]
    }

    fn insert_zero(&mut self) {
        self.stacks[0].target.fill(0);
        self.stacks[0].actual.fill(0.into());
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as StackCoeff, stack)
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as StackCoeff, stack)?;
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, stack) in self.stacks.iter_mut().enumerate() {
            f(i as StackCoeff, stack)
        }
    }

    fn has_tuning_for(&self, _: StackCoeff) -> bool {
        true
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let quot = offset.div_euclid(n);
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
}

/// Marker trait of neighbourhoods that can return a note for every offset.
pub trait CompleteNeigbourhood<T: IntervalBasis>: Neighbourhood<T> {
    fn write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) {
        self.try_write_relative_stack(target, offset);
    }

    fn get_relative_stack(&self, offset: StackCoeff) -> Stack<T> {
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
    fn period_keys(&self) -> StackCoeff {
        self.period().key_distance()
    }
}

impl<T: IntervalBasis> PeriodicNeighbourhood<T> for PeriodicComplete<T> {}
