use std::{marker::PhantomData, sync::mpsc, time::Instant};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, ToProcess, ToStrategy},
};

use super::r#trait::Strategy;

pub struct IntervalSolution<T: StackType> {
    /// MIDI key numbers together with their tuning relative to the reference note
    pub intervals: Vec<(u8, Stack<T>)>,
    /// MIDI key number of the reference note (which is not necessarily present in
    /// [Self::intervals])
    pub reference: u8,
}

pub trait IntervalStrategy<T: StackType> {
    fn solve(&mut self, keys: &[KeyState; 128]) -> Option<IntervalSolution<T>>;
}

pub trait AnchorStrategy<T: StackType> {
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        intervals: IntervalSolution<T>,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;
}

pub struct TwoStep<T: StackType, I: IntervalStrategy<T>, A: AnchorStrategy<T>> {
    _phantom: PhantomData<T>,
    interval_strategy: I,
    anchor_strategy: A,
}

impl<T: StackType, I: IntervalStrategy<T>, A: AnchorStrategy<T>> TwoStep<T, I, A> {
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        match self.interval_strategy.solve(keys) {
            Some(intervals) => self
                .anchor_strategy
                .solve(keys, tunings, intervals, time, forward),
            None {} => false,
        }
    }
}

impl<T: StackType, I: IntervalStrategy<T>, A: AnchorStrategy<T>> Strategy<T> for TwoStep<T, I, A> {
    fn note_on(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        _note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        self.solve(keys, tunings, time, forward)
    }

    fn note_off(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        _notes: &[u8],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        self.solve(keys, tunings, time, forward)
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        self.anchor_strategy.handle_msg(keys, tunings, msg, forward)
    }
}
