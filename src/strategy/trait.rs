use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::mpsc,
    time::Instant,
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, IsStrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromStrategy, ReceiveMsg, ToStrategy},
    reference::Reference,
    util::readerwriter::{ConcreteReader, ConcreteReaderWriter, Reader, ReaderWriter},
};

/// Why these are not simply variants of [ToStrategy]: I want to expose them to users, to construct
/// [crate::bindable::Bindings] in the configuration file, and [ToStrategy] doesn't belong there.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyAction {
    // SwitchToNeighbourhood(usize),
    IncrementNeighbourhoodIndex(isize),
    SetReferenceToLowest,
    SetReferenceToHighest,
    SetReferenceToCurrent,
    ToggleChordMatching,
    ToggleReanchor,
    Reset,
}

impl fmt::Display for StrategyAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StrategyAction::IncrementNeighbourhoodIndex(i) => {
                write!(f, "increment neighbourhood index by {i}")
            }
            StrategyAction::SetReferenceToLowest => {
                write!(f, "set reference to lowest sounding note")
            }
            StrategyAction::SetReferenceToHighest => {
                write!(f, "set reference to highest sounding note")
            }
            StrategyAction::SetReferenceToCurrent => {
                write!(f, "set reference to current chord's reference")
            }
            StrategyAction::ToggleChordMatching => write!(f, "toggle chord matching"),
            StrategyAction::ToggleReanchor => {
                write!(f, "toggle re-setting of the reference on chord match")
            }
            StrategyAction::Reset => write!(f, "reset"),
        }
    }
}

pub trait StrategyAdaptor<T: StackType>:
    Reader<[KeyState; 128]> + ReaderWriter<[Stack<T>; 128]>
{
    fn send(&self, msg: FromStrategy<T>) -> bool;
    fn read_key_states(&self) -> impl Deref<Target = [KeyState; 128]> {
        <Self as Reader<[KeyState; 128]>>::read(self)
    }
    fn read_tunings(&self) -> impl Deref<Target = [Stack<T>; 128]> {
        <Self as Reader<[Stack<T>; 128]>>::read(self)
    }
    fn write_tunings(&self) -> impl DerefMut<Target = [Stack<T>; 128]> {
        <Self as ReaderWriter<[Stack<T>; 128]>>::write(self)
    }
}

#[derive(Clone)]
pub struct ConcreteStrategyAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromStrategy<T>>,
    pub key_states: ConcreteReader<[KeyState; 128]>,
    pub tunings: ConcreteReaderWriter<[Stack<T>; 128]>,
}

impl<T: StackType> Reader<[KeyState; 128]> for ConcreteStrategyAdaptor<T> {
    fn read(&self) -> impl Deref<Target = [KeyState; 128]> {
        self.key_states.read()
    }
}

impl<T: StackType> Reader<[Stack<T>; 128]> for ConcreteStrategyAdaptor<T> {
    fn read(&self) -> impl Deref<Target = [Stack<T>; 128]> {
        self.tunings.read()
    }
}

impl<T: StackType> ReaderWriter<[Stack<T>; 128]> for ConcreteStrategyAdaptor<T> {
    fn write(&self) -> impl DerefMut<Target = [Stack<T>; 128]> {
        self.tunings.write()
    }
}

impl<T: StackType> StrategyAdaptor<T> for ConcreteStrategyAdaptor<T> {
    fn send(&self, msg: FromStrategy<T>) -> bool {
        self.forward.send(msg).is_ok()
    }
}

pub trait Strategy<T: StackType>: ExtractConfig<Self::Config> {
    type Msg;

    type Config: IsStrategyConfig<T, Realized = Self>;

    fn new(config: Self::Config) -> Self;

    /// returns true iff further [Strategy::step]s are needed.
    fn start(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool;

    fn stop(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>);

    /// returns true iff further [Strategy::step]s are needed.
    fn note_on(&mut self, note: u8, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn note_off(&mut self, note: u8, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn set_tuning_reference(
        &mut self,
        reference: Reference<T>,
        time: Instant,
        adaptor: &impl StrategyAdaptor<T>,
    ) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &impl StrategyAdaptor<T>) -> bool;

    /// returns true iff further [Strategy::step]s are needed.
    fn step(&mut self, adaptor: &impl StrategyAdaptor<T>) -> bool;

    /// should return only the "custom messages" for this strategy.
    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg>;

    /// This is intended to run in its own thread.
    fn receive_solve_loop(
        &mut self,
        to_strategy_rx: mpsc::Receiver<ToStrategy<T>>,
        adaptor: &impl StrategyAdaptor<T>,
    ) -> Self::Config {
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
                Some(ToStrategy::Start { time }) => continue_solving = self.start(time, adaptor),
                Some(ToStrategy::Stop { time }) => {
                    self.stop(time, adaptor);
                    break;
                }
                Some(ToStrategy::NoteOn { note, time }) => {
                    continue_solving = self.note_on(note, time, adaptor)
                }
                Some(ToStrategy::NoteOff { note, time }) => {
                    continue_solving = self.note_off(note, time, adaptor)
                }
                Some(ToStrategy::SetTuningReference { reference, time }) => {
                    continue_solving = self.set_tuning_reference(reference, time, adaptor)
                }
                Some(msg) => {
                    if let Some(x) = Self::filter_to_strategy(msg) {
                        continue_solving = self.receive_msg(x, adaptor);
                    }
                }
            }
        }

        self.extract_config()
    }
}
