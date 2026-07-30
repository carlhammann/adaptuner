use std::{marker::PhantomData, sync::mpsc, time::Instant};

use crate::{
    adaptors::lock_levels::{
        ActiveStrategyIndexLevel, KeyStateLevel, ReferenceLevel, StrategyConfigLevel,
        TuningReferenceLevel, TuningStateLevel,
    },
    bindable::BindableStrategyAction,
    config::IsStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{FromProcess, FromStrategy, ToStrategy},
    process::r#trait::ProcessAdaptor,
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

pub type StrategyAdaptor<T, S, P, L> = OrderedLocks<(PhantomData<T>, S), P, L>;

impl<T: StackType, S: Strategy<T>, P: ProcessAdaptor<StackType = T>, L: Nat>
    StrategyAdaptor<T, S, P, L>
{
    pub fn send(&self, msg: FromStrategy<T>) {
        unsafe { self.inner() }.send(FromProcess::FromStrategy(msg));
    }
}

pub trait Strategy<T: StackType>: Sized {
    type Msg;

    type Config: IsStrategyConfig<T>;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [Strategy::step]s are needed.
    fn start<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    fn stop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero>;

    /// This function should always be called between [Self::stop] and [Self::start]. It should set
    /// everything to the starting values from the configuration in the adaptor.
    ///
    /// This function is deprecated because it should be a restart of the strategy from the process
    /// loop.
    #[deprecated]
    fn reset<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero>;

    /// returns true iff further [Strategy::step]s are needed.
    fn note_on<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn note_off<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        note: u8,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn update_tuning_reference<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn consider<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        stack: Stack<T>,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn receive_msg<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        msg: Self::Msg,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// returns true iff further [Strategy::step]s are needed.
    fn step<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// should return only the "custom messages" for this strategy.
    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig:reacts_to_bound] returns true. Should return true iff
    /// further [Self::step]s are needed.
    fn handle_bound_action<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> (bool, StrategyAdaptor<T, Self, P, Zero>);

    /// This is intended to run in its own thread.
    fn receive_solve_loop<P: ProcessAdaptor<StackType = T>>(
        &mut self,
        needs_steps_at_first_iteration: bool,
        to_strategy_rx: mpsc::Receiver<ToStrategy<T>>,
        mut adaptor: StrategyAdaptor<T, Self, P, Zero>,
    ) -> StrategyAdaptor<T, Self, P, Zero> {
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
