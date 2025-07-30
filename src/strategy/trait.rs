use std::{collections::VecDeque, fmt, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, StrategyConfig},
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
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
}

impl fmt::Display for StrategyAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // StrategyAction::SwitchToNeighbourhood(i) => {
            //     write!(f, "switch to neighbourhood {}", i + 1)
            // }
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

pub trait Strategy<T: StackType>: ExtractConfig<StrategyConfig<T>> {
    /// expects the effect of the "note on" event to be already reflected in `keys`.
    ///
    /// Returns the tuning of the note that was turned on, if it was successfully tuned.
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)>;

    /// expects the effect of the "note off" event to be alead reflected in `keys`
    fn note_off(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool;

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool;

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    );
}
