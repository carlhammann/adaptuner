use std::collections::BTreeMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    bindable::Bindable,
    interval::{
        stacktype::r#trait::{IntervalBasis, NamedInterval, StackType},
        temperament::TemperamentDefinition,
    },
    strategy::{
        harmony::{chordlist::ChordListConfig, r#trait::HarmonyStrategy},
        melody::{neighbourhoods::StaticNeighbourhoodsAsMelodyConfig, r#trait::MelodyStrategy},
        r#trait::{Strategy, StrategyAction},
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
};

pub trait FromConfigAndState<C, S> {
    fn initialise(config: C, state: S) -> Self;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
    version: AdaptunerVersion,
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub named_intervals: Vec<NamedInterval<T>>,
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
    StaticNeighbourhoods(StaticNeighbourhoodsConfig<T>),
    TwoStep {
        name: String,
        description: String,
        harmony: HarmonyStrategyConfig<T>,
        melody: MelodyStrategyConfig<T>,
        bindings: BTreeMap<Bindable, StrategyAction>,
    },
}

impl<T: IntervalBasis> StrategyConfig<T> {
    pub fn bindings(&self) -> &BTreeMap<Bindable, StrategyAction> {
        match self {
            StrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsConfig {
                bindings, ..
            }) => &bindings,
            StrategyConfig::TwoStep { .. } => todo!(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            StrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsConfig { name, .. }) => &name,
            StrategyConfig::TwoStep { name, .. } => name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            StrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsConfig {
                description,
                ..
            }) => &description,
            StrategyConfig::TwoStep { description, .. } => description,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyStrategyConfig<T: IntervalBasis> {
    ChordList(ChordListConfig<T>),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum MelodyStrategyConfig<T: IntervalBasis> {
    StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig<T>),
}

/// Marker trait for strategy configuration types. [StrategyConfig] does not implement this, but
/// the types it "wraps" should.
pub trait IsStrategyConfig<T: StackType> {
    type Realized: Strategy<T, Config = Self>;

    fn realize(self) -> Self::Realized
    where
        Self: Sized,
    {
        Self::Realized::new(self)
    }
}

pub trait IsHarmonyStrategyConfig<T: StackType> {
    type Realized: HarmonyStrategy<T, Config = Self>;
    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T>;
}

pub trait IsMelodyStrategyConfig<T: StackType> {
    type Realized: MelodyStrategy<T, Config = Self>;
    fn as_melody_strategy_config(self) -> MelodyStrategyConfig<T>;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct GuiConfig {
    // pub lattice_window: LatticeWindowConfig,
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
