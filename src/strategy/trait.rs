use std::{fmt, sync::mpsc, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::StrategyConfig,
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, ToStrategy},
};

/// Why these are not simply variants of [ToStrategy]: I want to expose them to users, to construct
/// [crate::bindable::Bindings] in the configuration file, and [ToStrategy] doesn't belong there.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyAction {
    IncrementNeighbourhoodIndex(isize),
    SetReferenceToLowest,
    SetReferenceToHighest,
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
        }
    }
}

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`.
    ///
    /// May only send [FromProcess::FromStrategy] messages.
    ///
    /// Returns the tuning of the note that was turned on, if it was successfully tuned.
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)>;

    /// expects the effect of the "note off" event to be alead reflected in `keys`
    ///
    /// May only send [FromProcess::FromStrategy] messages.
    ///
    /// There are possibly more than one note off events becaus a pedal release my simultaneously
    /// switch off many notes.
    fn note_off(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        notes: &[u8],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    /// May only send [FromProcess::FromStrategy] messages.
    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    );

    fn extract_config(&self) -> StrategyConfig<T>;
}
