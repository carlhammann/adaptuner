use std::fmt;

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, StrategyConfig},
    interval::stacktype::r#trait::StackType,
    msg::{FromStrategy, ReceiveMsg, SendMsg, ToStrategy},
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

pub trait Strategy<T: StackType>:
    ReceiveMsg<ToStrategy<T>> + SendMsg<FromStrategy<T>> + ExtractConfig<StrategyConfig<T>>
{
}
