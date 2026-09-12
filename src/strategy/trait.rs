use std::{marker::PhantomData, sync::mpsc, time::Instant};

use crate::{
    adaptors::{
        lock_levels::{
            ActiveStrategyIndexLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel,
            TuningReferenceLevel, TuningStateLevel,
        },
        ConcreteLocks,
    },
    bindable::BindableStrategyAction,
    config::IsStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{FromProcess, FromStrategy, ToStrategy},
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed, WriteAllowed, Zero},
};

impl<T: StackType, S: Strategy<T>> ReadAllowed<StrategyConfigLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> ReadAllowed<ActiveStrategyIndexLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> ReadAllowed<KeyStateLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> ReadAllowed<TuningStateLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> ReadAllowed<TuningReferenceLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> ReadAllowed<ReferenceLevel> for (PhantomData<T>, S) {}

impl<T: StackType, S: Strategy<T>> WriteAllowed<TuningStateLevel> for (PhantomData<T>, S) {}
impl<T: StackType, S: Strategy<T>> WriteAllowed<ReferenceLevel> for (PhantomData<T>, S) {}

pub type StrategyAdaptor<T, S, L> = OrderedLocks<(PhantomData<T>, S), ConcreteLocks<T>, L>;

impl<T: StackType, S: Strategy<T>, L: Nat> StrategyAdaptor<T, S, L> {
    pub fn send(&self, msg: FromStrategy<T>) {
        let _ = unsafe { self.inner() }
            .from_process_tx
            .send(FromProcess::FromStrategy(msg));
    }
}

pub trait Strategy<T: StackType>: Sized {
    type Msg;

    type Config: IsStrategyConfig<T>;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [Strategy::step]s are needed.
    fn start(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    fn stop(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> StrategyAdaptor<T, Self, Zero>;

    /// returns true iff further [Strategy::step]s are needed.
    fn note_on(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn note_off(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn update_tuning_reference(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn consider(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn step(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// should return only the "custom messages" for this strategy.
    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig:reacts_to_bound] returns true. Should return true iff
    /// further [Self::step]s are needed.
    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>);

    /// This is intended to run in its own thread.
    fn receive_solve_loop(
        &mut self,
        needs_steps_at_first_iteration: bool,
        to_strategy_rx: mpsc::Receiver<ToStrategy<T>>,
        mut adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> StrategyAdaptor<T, Self, Zero> {
        let mut continue_solving = needs_steps_at_first_iteration;
        let mut last_msg = None {};
        loop {
            if continue_solving {
                (continue_solving, adaptor) = self.step(adaptor);
                if let Ok(msg) = to_strategy_rx.try_recv() {
                    last_msg = Some(msg);
                }
            } else {
                if let Ok(msg) = to_strategy_rx.recv() {
                    last_msg = Some(msg);
                } else {
                    break;
                }
            }

            match last_msg.take() {
                None {} => {}
                Some(ToStrategy::NoteOn { note, time }) => {
                    (continue_solving, adaptor) = self.note_on(note, time, adaptor)
                }
                Some(ToStrategy::NoteOff { note, time }) => {
                    (continue_solving, adaptor) = self.note_off(note, time, adaptor)
                }
                Some(ToStrategy::UpdateTuningReference { time }) => {
                    (continue_solving, adaptor) = self.update_tuning_reference(time, adaptor)
                }
                Some(ToStrategy::Consider { stack, time }) => {
                    (continue_solving, adaptor) = self.consider(stack, time, adaptor)
                }
                Some(ToStrategy::Start { time }) => {
                    (continue_solving, adaptor) = self.start(time, adaptor)
                }
                Some(ToStrategy::Stop { time }) => {
                    adaptor = self.stop(time, adaptor);
                    break;
                }
                Some(ToStrategy::BoundAction { action, time }) => {
                    (continue_solving, adaptor) = self.handle_bound_action(action, time, adaptor)
                }
                Some(msg) => {
                    if let Some(x) = Self::filter_to_strategy(msg) {
                        (continue_solving, adaptor) = self.receive_msg(x, adaptor);
                    }
                }
            }
        }

        adaptor
    }
}
