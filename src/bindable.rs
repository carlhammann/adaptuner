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
pub enum MidiBindable {
    SostenutoPedalDown,
    SostenutoPedalUp,
    SoftPedalDown,
    SoftPedalUp,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[serde(untagged)]
pub enum Bindable {
    Midi(MidiBindable),
    #[serde(
        deserialize_with = "deserialize_egui_key",
        serialize_with = "serialize_egui_key"
    )]
    KeyPress(egui::Key),
}

impl fmt::Display for MidiBindable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MidiBindable::SostenutoPedalDown => write!(f, "sostenuto pedal down"),
            MidiBindable::SostenutoPedalUp => write!(f, "sostenuto pedal up"),
            MidiBindable::SoftPedalDown => write!(f, "soft pedal down"),
            MidiBindable::SoftPedalUp => write!(f, "soft pedal up"),
        }
    }
}

impl fmt::Display for Bindable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Bindable::Midi(b) => b.fmt(f),
            Bindable::KeyPress(key) => write!(f, "key press on {}", key.symbol_or_name()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bindings<K: Ord>(BTreeMap<K, StrategyAction>);

impl Bindings<Bindable> {
    pub fn only_midi(&self) -> Bindings<MidiBindable> {
        let Bindings(m) = self;
        let mut res = BTreeMap::new();
        m.iter().for_each(|(k, v)| match k {
            Bindable::Midi(k) => {
                res.insert(*k, *v);
            }
            _ => {}
        });
        Bindings(res)
    }
}

impl<K: Ord> Bindings<K> {
    pub fn get(&self, bindable: &K) -> Option<&StrategyAction> {
        let Bindings(m) = self;
        m.get(bindable)
    }

    pub fn insert(&mut self, bindable: K, action: StrategyAction) -> Option<StrategyAction> {
        let Bindings(m) = self;
        m.insert(bindable, action)
    }

    pub fn remove(&mut self, bindable: &K) -> Option<StrategyAction> {
        let Bindings(m) = self;
        m.remove(bindable)
    }

    pub fn iter(&mut self) -> std::collections::btree_map::Iter<'_, K, StrategyAction> {
        let Bindings(m) = self;
        m.iter()
    }
}
