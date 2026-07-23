use std::fmt;

use eframe::egui;
use serde_derive::{Deserialize, Serialize};

use crate::{
    custom_serde::common::{deserialize_egui_key, serialize_egui_key},
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum BindableEvent {
    SostenutoPedalDown,
    SostenutoPedalUp,
    SoftPedalDown,
    SoftPedalUp,
    #[serde(
        deserialize_with = "deserialize_egui_key",
        serialize_with = "serialize_egui_key"
    )]
    KeyPress(egui::Key),
}

impl fmt::Display for BindableEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BindableEvent::SostenutoPedalDown => write!(f, "sostenuto pedal down"),
            BindableEvent::SostenutoPedalUp => write!(f, "sostenuto pedal up"),
            BindableEvent::SoftPedalDown => write!(f, "soft pedal down"),
            BindableEvent::SoftPedalUp => write!(f, "soft pedal up"),
            BindableEvent::KeyPress(key) => write!(f, "key press on {}", key.symbol_or_name()),
        }
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum BindableStrategyAction {
    IncrementNeighbourhoodIndex(isize),
    SetReferenceToLowest,
    SetReferenceToHighest,
    SetReferenceToCurrent,
    ToggleChordMatching,
    ToggleReanchor,
    Reset,
}

impl fmt::Display for BindableStrategyAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BindableStrategyAction::IncrementNeighbourhoodIndex(i) => {
                write!(f, "skip to scale at offset {i}")
            }
            BindableStrategyAction::SetReferenceToLowest => {
                write!(f, "set reference to lowest sounding note")
            }
            BindableStrategyAction::SetReferenceToHighest => {
                write!(f, "set reference to highest sounding note")
            }
            BindableStrategyAction::SetReferenceToCurrent => {
                write!(f, "set reference to current chord's reference")
            }
            BindableStrategyAction::ToggleChordMatching => write!(f, "toggle chord matching"),
            BindableStrategyAction::ToggleReanchor => {
                write!(f, "toggle re-setting of the reference on chord match")
            }
            BindableStrategyAction::Reset => write!(f, "reset"),
        }
    }
}
