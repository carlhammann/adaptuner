use std::{
    ops::{Deref, DerefMut},
    sync::mpsc,
    time::Instant,
};

use crate::{
    adaptors::{ChangeTunings, ViewKeyStates, ViewTunings},
    bindable::BindableStrategyAction,
    config::IsStrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::{FromStrategy, ToStrategy},
    reference::Reference,
};

pub trait StrategyAdaptor<T: StackType>: ViewKeyStates + ViewTunings<T> + ChangeTunings<T> {
    fn send(&self, msg: FromStrategy<T>) -> bool;
    fn reference(&self) -> impl Deref<Target = Stack<T>>;
    fn reference_mut(&self) -> impl DerefMut<Target = Stack<T>>;
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>>;
}

pub trait Strategy<T: StackType, A: StrategyAdaptor<T>> {
    type Msg;

    type Config: IsStrategyConfig<T>;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [Strategy::step]s are needed.
    fn start(&mut self, time: Instant, adaptor: &A) -> bool;

    fn stop(&mut self, time: Instant, adaptor: &A);

    /// This function should always be called between [Self::stop] and [Self::start]. It should set
    /// everything to the starting values from the configuration in the adaptor.
    fn reset(&mut self, adaptor: &A);

    /// returns true iff further [Strategy::step]s are needed.
    fn note_on(&mut self, note: u8, time: Instant, adaptor: &A) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn note_off(&mut self, note: u8, time: Instant, adaptor: &A) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn update_tuning_reference(&mut self, time: Instant, adaptor: &A) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn consider(&mut self, stack: Stack<T>, time: Instant, adaptor: &A) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn step(&mut self, adaptor: &A) -> bool;

    /// should return only the "custom messages" for this strategy.
    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg>;

    /// Should only do something if [StrategyConfig:reacts_to_bound] returns true. Should return true iff
    /// further [Self::step]s are needed.
    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: &A,
    ) -> bool;

    /// This is intended to run in its own thread.
    fn receive_solve_loop(&mut self, to_strategy_rx: mpsc::Receiver<ToStrategy<T>>, adaptor: &A) {
        let mut continue_solving = false;
        let mut last_msg = None {};
        loop {
            if continue_solving {
                continue_solving = self.step(adaptor);
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
                    continue_solving = self.note_on(note, time, adaptor)
                }
                Some(ToStrategy::NoteOff { note, time }) => {
                    continue_solving = self.note_off(note, time, adaptor)
                }
                Some(ToStrategy::UpdateTuningReference { time }) => {
                    continue_solving = self.update_tuning_reference(time, adaptor)
                }
                Some(ToStrategy::Consider { stack, time }) => {
                    continue_solving = self.consider(stack, time, adaptor)
                }
                Some(ToStrategy::Start { time }) => continue_solving = self.start(time, adaptor),
                Some(ToStrategy::Stop { time }) => {
                    self.stop(time, adaptor);
                    break;
                }
                Some(ToStrategy::BoundAction { action, time }) => {
                    continue_solving = self.handle_bound_action(action, time, adaptor)
                }
                Some(msg) => {
                    if let Some(x) = Self::filter_to_strategy(msg) {
                        continue_solving = self.receive_msg(x, adaptor);
                    }
                }
            }
        }
    }
}
