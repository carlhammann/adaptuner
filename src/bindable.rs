use std::{collections::BTreeMap, fmt};

use eframe::egui;
use serde_derive::{Deserialize, Serialize};

use crate::{
    custom_serde::common::{deserialize_egui_key, serialize_egui_key},
    strategy::r#trait::StrategyAction,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum Bindable {
    SostenutoPedalDown,
    SostenutoPedalUp,
    SoftPedalDown,
    SoftPedalUp,
    #[serde(
        deserialize_with = "deserialize_egui_key",
        serialize_with = "serialize_egui_key"
    )]
    KeyDown(egui::Key),
}

impl fmt::Display for Bindable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Bindable::SostenutoPedalDown => write!(f, "sostenuto pedal down"),
            Bindable::SostenutoPedalUp => write!(f, "sostenuto pedal up"),
            Bindable::SoftPedalDown => write!(f, "soft pedal down"),
            Bindable::SoftPedalUp => write!(f, "soft pedal up"),
            Bindable::KeyDown(key) => write!(f, "key press on {}", key.symbol_or_name()),
        }
    }
}

pub type Bindings = BTreeMap<Bindable, StrategyAction>;
