use std::{marker::PhantomData, sync::mpsc};

use crate::time::Instant;

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

    /// Processes one strategy message.
    ///
    /// Returns whether further [`Strategy::step`] calls are required.
    ///
    /// This is the synchronous core used by both the native and WASM
    /// runtimes.
    fn process_message(
        &mut self,
        msg: ToStrategy<T>,
        adaptor: StrategyAdaptor<T, Self, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, Zero>) {
        match msg {
            ToStrategy::NoteOn { note, time } => self.note_on(note, time, adaptor),

            ToStrategy::NoteOff { note, time } => self.note_off(note, time, adaptor),

            ToStrategy::UpdateTuningReference { time } => {
                self.update_tuning_reference(time, adaptor)
            }

            ToStrategy::Consider { stack, time } => self.consider(stack, time, adaptor),

            ToStrategy::Start { time } => self.start(time, adaptor),

            ToStrategy::Stop { time } => {
                let adaptor = self.stop(time, adaptor);
                (false, adaptor)
            }

            ToStrategy::BoundAction { action, time } => {
                self.handle_bound_action(action, time, adaptor)
            }

            msg => match Self::filter_to_strategy(msg) {
                Some(msg) => self.receive_msg(msg, adaptor),
                None => (false, adaptor),
            },
        }
    }

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
                match to_strategy_rx.recv() {
                    Ok(msg) => last_msg = Some(msg),
                    Err(_) => break,
                }
            }

            match last_msg.take() {
                None {} => {}

                Some(msg) => {
                    let is_stop = matches!(msg, ToStrategy::Stop { .. });

                    (continue_solving, adaptor) = self.process_message(msg, adaptor);

                    if is_stop {
                        break;
                    }
                }
            }
        }

        adaptor
    }
}
