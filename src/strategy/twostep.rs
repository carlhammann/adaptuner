use std::{sync::mpsc, time::Instant};

use crate::{
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, ToStrategy},
    reference::Reference,
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
    interval_strategy: I,
    anchor_strategy: A,
    global_reference: Reference<T>,
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
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)> {
        if self.solve(keys, tunings, time, forward) {
            let stack = &tunings[note as usize];
            Some((
                stack.absolute_semitones(self.global_reference.c4_semitones()),
                stack,
            ))
        } else {
            None {}
        }
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
