use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::mpsc,
    time::Instant,
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, IsStrategyConfig},
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
    process::r#trait::StackWithTuning,
    reference::Reference,
    util::readerwriter::{ConcreteReader128, ConcreteReaderWriter128, Reader128, ReaderWriter128},
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
    Reader128<KeyState> + ReaderWriter128<StackWithTuning<T>>
{
    fn send(&self, msg: FromStrategy<T>) -> bool;
    fn read_key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        <Self as Reader128<KeyState>>::read(self, i)
    }
    fn read_tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        <Self as Reader128<StackWithTuning<T>>>::read(self, i)
    }
    fn write_tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        <Self as ReaderWriter128<StackWithTuning<T>>>::write(self, i)
    }
}

#[derive(Clone)]
pub struct ConcreteStrategyAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromStrategy<T>>,
    pub key_states: ConcreteReader128<KeyState>,
    pub tunings: ConcreteReaderWriter128<StackWithTuning<T>>,
}

impl<T: StackType> Reader128<KeyState> for ConcreteStrategyAdaptor<T> {
    fn read(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.key_states.read(i)
    }

    fn read_all(&self) -> impl Deref<Target = [impl AsRef<KeyState>; 128]> {
        self.key_states.read_all()
    }
}

impl<T: StackType> Reader128<StackWithTuning<T>> for ConcreteStrategyAdaptor<T> {
    fn read(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings.read(i)
    }

    fn read_all(&self) -> impl Deref<Target = [impl AsRef<StackWithTuning<T>>; 128]> {
        self.tunings.read_all()
    }
}

impl<T: StackType> ReaderWriter128<StackWithTuning<T>> for ConcreteStrategyAdaptor<T> {
    fn write(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.tunings.write(i)
    }

    fn write_all(&self) -> impl DerefMut<Target = [impl AsMut<StackWithTuning<T>>; 128]> {
        self.tunings.write_all()
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

    fn reset(&mut self, time: Instant, adaptor: &impl StrategyAdaptor<T>) -> bool;

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
