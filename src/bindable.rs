use std::fmt;

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum Bindable {
    SostenutoPedalDown,
    SostenutoPedalUp,
    SoftPedalDown,
    SoftPedalUp,
}

impl fmt::Display for Bindable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Bindable::SostenutoPedalDown => write!(f, "sostenuto pedal down"),
            Bindable::SostenutoPedalUp => write!(f, "sostenuto pedal up"),
            Bindable::SoftPedalDown => write!(f, "soft pedal down"),
            Bindable::SoftPedalUp => write!(f, "soft pedal up"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct OneBinding<X> {
    pub on_down: Option<X>,
    pub on_up: Option<X>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Bindings<X> {
    pub sostenuto_pedal: OneBinding<X>,
    pub soft_pedal: OneBinding<X>,
}

impl<X> Bindings<X> {
    pub fn set(&mut self, bindable: Bindable, action: Option<X>) {
        match bindable {
            Bindable::SostenutoPedalDown => self.sostenuto_pedal.on_down = action,
            Bindable::SostenutoPedalUp => self.sostenuto_pedal.on_up = action,
            Bindable::SoftPedalDown => self.soft_pedal.on_down = action,
            Bindable::SoftPedalUp => self.soft_pedal.on_up = action,
        }
    }

    pub fn get(&self, bindable: Bindable) -> Option<&X> {
        match bindable {
            Bindable::SostenutoPedalDown => self.sostenuto_pedal.on_down.as_ref(),
            Bindable::SostenutoPedalUp => self.sostenuto_pedal.on_up.as_ref(),
            Bindable::SoftPedalDown => self.soft_pedal.on_down.as_ref(),
            Bindable::SoftPedalUp => self.soft_pedal.on_up.as_ref(),
        }
    }

    pub fn get_mut(&mut self, bindable: Bindable) -> &mut Option<X> {
        match bindable {
            Bindable::SostenutoPedalDown => &mut self.sostenuto_pedal.on_down,
            Bindable::SostenutoPedalUp => &mut self.sostenuto_pedal.on_up,
            Bindable::SoftPedalDown => &mut self.soft_pedal.on_down,
            Bindable::SoftPedalUp => &mut self.soft_pedal.on_up,
        }
    }
}
