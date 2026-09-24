use std::collections::BTreeMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    bindable::{BindableEvent, BindableProcessAction, BindableStrategyAction},
    gui::lattice::LatticeWindowConfig,
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, NamedInterval, StackType},
        temperament::TemperamentDefinition,
    },
    notename::NoteNameStyle,
    reference::Reference,
    strategy::{
        harmony::{chordlist::ChordListConfig, springs::HarmonySpringsConfig},
        melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
    pub version: AdaptunerVersion,
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub named_intervals: Vec<NamedInterval<T>>,
    pub tuning_reference: Reference<T>,
    pub strategies: Vec<StrategyConfig<T>>, // must be non-empty
    pub backend: BackendConfig,
    pub gui: GuiConfig,
}

/// This (de) serializes as the current version string.
pub struct AdaptunerVersion;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyConfig<T: IntervalBasis> {
    #[serde(rename_all = "kebab-case")]
    StaticNeighbourhoods {
        name: String,
        description: String,
        config: StaticNeighbourhoodsConfig<T>,
        bindings: BTreeMap<BindableEvent, BindableProcessAction>,
    },
    #[serde(rename_all = "kebab-case")]
    TwoStep {
        name: String,
        description: String,
        harmony: HarmonyStrategyConfig<T>,
        melody: MelodyStrategyConfig<T>,
        bindings: BTreeMap<BindableEvent, BindableProcessAction>,
        melody_harmony_coordination: MelodyHarmonyCoordinationConfig,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct MelodyHarmonyCoordinationConfig {
    pub reanchor: bool,
    pub group_ms: u64,
    pub tune_wait_us: u64,
}

impl<T: IntervalBasis> StrategyConfig<T> {
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            StrategyConfig::StaticNeighbourhoods { name, .. } => name,
            StrategyConfig::TwoStep { name, .. } => name,
        }
    }

    #[inline]
    pub fn name_mut(&mut self) -> &mut String {
        match self {
            StrategyConfig::StaticNeighbourhoods { name, .. } => name,
            StrategyConfig::TwoStep { name, .. } => name,
        }
    }

    #[inline]
    pub fn description(&self) -> &str {
        match self {
            StrategyConfig::StaticNeighbourhoods { description, .. } => description,
            StrategyConfig::TwoStep { description, .. } => description,
        }
    }

    #[inline]
    pub fn description_mut(&mut self) -> &mut String {
        match self {
            StrategyConfig::StaticNeighbourhoods { description, .. } => description,
            StrategyConfig::TwoStep { description, .. } => description,
        }
    }

    pub fn reacts_to_bound(&self, action: BindableStrategyAction) -> bool {
        match self {
            StrategyConfig::StaticNeighbourhoods { .. } => match action {
                BindableStrategyAction::IncrementNeighbourhoodIndex(_)
                | BindableStrategyAction::SetReferenceToLowest
                | BindableStrategyAction::SetReferenceToHighest => true,
                _ => false,
            },
            StrategyConfig::TwoStep {
                harmony, melody, ..
            } => {
                (match harmony {
                    HarmonyStrategyConfig::ChordList(_) => match action {
                        _ => false,
                    },
                    HarmonyStrategyConfig::Springs(_) => match action {
                        _ => false,
                    },
                    HarmonyStrategyConfig::List(_) => match action {
                        _ => false,
                    },
                }) || (match melody {
                    MelodyStrategyConfig::StaticNeighbourhoods(_) => match action {
                        BindableStrategyAction::IncrementNeighbourhoodIndex(_)
                        | BindableStrategyAction::SetReferenceToLowest
                        | BindableStrategyAction::SetReferenceToHighest
                        | BindableStrategyAction::SetReferenceToCurrent => true,
                        _ => false,
                    },
                })
            }
        }
    }

    #[inline]
    pub fn bindings(&self) -> &BTreeMap<BindableEvent, BindableProcessAction> {
        match self {
            StrategyConfig::StaticNeighbourhoods { bindings, .. } => bindings,
            StrategyConfig::TwoStep { bindings, .. } => bindings,
        }
    }

    #[inline]
    pub fn bindings_mut(&mut self) -> &mut BTreeMap<BindableEvent, BindableProcessAction> {
        match self {
            StrategyConfig::StaticNeighbourhoods { bindings, .. } => bindings,
            StrategyConfig::TwoStep { bindings, .. } => bindings,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyStrategyConfig<T: IntervalBasis> {
    ChordList(ChordListConfig<T>),
    Springs(HarmonySpringsConfig<T>),
    /// Must never be nested, and must contain each type of [HarmonyStrategyConfig] at most once.
    List(Vec<HarmonyStrategyConfig<T>>),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum MelodyStrategyConfig<T: IntervalBasis> {
    StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig<T>),
}

/// Marker trait for strategy configuration types. [StrategyConfig] does not implement this, but
/// the types it "wraps" should.
pub trait IsStrategyConfig<T: StackType>: Clone {}

pub trait IsHarmonyStrategyConfig<T: StackType>: Clone {
    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T>;
}

pub trait IsMelodyStrategyConfig<T: StackType>: Clone {
    fn initial_scale_reference(&self) -> Option<&Stack<T>>;
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct GuiConfig {
    pub notenamestyle: NoteNameStyle,
    pub lattice: LatticeWindowConfig,
    // pub tuning_editor: TuningEditorConfig,
    // pub reference_editor: ReferenceEditorConfig,
    pub latency_mean_over: usize,
    pub use_cent_values: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum BackendConfig {
    Pitchbend12(Pitchbend12Config),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct MidiInputConfig {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct MidiOutputConfig {}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Named<X> {
    pub name: String,
    pub named: X,
}
