use std::{fmt, sync::mpsc, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, IsStrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromStrategy, ReceiveMsg, ToStrategy},
    reference::Reference,
    util::readerwriter::{Reader, ReaderWriter},
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

pub trait Strategy<T: StackType>: ReceiveMsg<Self::Msg> + ExtractConfig<Self::Config> {
    type Msg;

    type Config: IsStrategyConfig<T, Realized = Self>;

    fn new(
        config: Self::Config,
        forward: mpsc::Sender<FromStrategy<T>>,
        key_states: Reader<[KeyState; 128]>,
        tunings: ReaderWriter<[Stack<T>; 128]>,
    ) -> Self;

    fn note_on(&mut self, note: u8, time: Instant);
    fn note_off(&mut self, note: u8, time: Instant);
    fn start(&mut self, time: Instant);
    fn stop(&mut self, time: Instant);
    fn set_tuning_reference(&mut self, reference: Reference<T>, time: Instant);

    /// should filter out the "custom messages" for this strategy.
    fn filter_to_strategy(msg: ToStrategy<T>) -> Option<Self::Msg>;

    fn receive_to_strategy(&mut self, msg: ToStrategy<T>) {
        match msg {
            ToStrategy::Start { time } => self.start(time),
            ToStrategy::Stop { time } => self.stop(time),
            ToStrategy::NoteOn { note, time } => self.note_on(note, time),
            ToStrategy::NoteOff { note, time } => self.note_off(note, time),
            ToStrategy::SetTuningReference { reference, time } => {
                self.set_tuning_reference(reference, time)
            }
            _ => {
                if let Some(x) = Self::filter_to_strategy(msg) {
                    self.receive_msg(x);
                }
            }
        }
    }
}
