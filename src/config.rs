use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    bindable::{Bindable, Bindings, MidiBindable},
    custom_serde::common::deserialize_nonempty,
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, NamedInterval, StackType},
        temperament::TemperamentDefinition,
    },
    neighbourhood::{PeriodicComplete, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::Strategy,
    },
};

pub trait FromConfigAndState<C, S> {
    fn initialise(config: C, state: S) -> Self;
}

pub trait ExtractConfig<C> {
    fn extract_config(&self) -> C;
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyConfig<T: IntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
}

impl<T: StackType> StrategyConfig<T> {
    pub fn realize(self) -> Box<dyn Strategy<T>> {
        match self {
            StrategyConfig::StaticTuning(config) => Box::new(StaticTuning::new(config)),
        }
    }
}

pub struct ProcessConfig<T: IntervalBasis> {
    pub strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
}

#[derive(Clone)]
pub struct GuiConfig {
    pub strategies: Vec<(StrategyNames, Bindings<Bindable>)>,
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
pub struct MidiInputConfig {
    // pub input: midir::MidiInput,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct MidiOutputConfig {
    // pub output: midir::MidiOutput,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub named_intervals: Vec<NamedInterval<T>>,
    pub strategies: Vec<ExtendedStrategyConfig<T>>,
    pub backend: BackendConfig,
}

impl<T: IntervalBasis> Config<T> {
    pub fn split(&self) -> (ProcessConfig<T>, GuiConfig, BackendConfig) {
        let mut process = Vec::with_capacity(self.strategies.len());
        let mut ui = Vec::with_capacity(self.strategies.len());
        self.strategies.iter().for_each(|esc| {
            let (sc, bs, ns) = esc.split();
            process.push((sc, bs.only_midi()));
            ui.push((ns, bs))
        });
        (
            ProcessConfig {
                strategies: process,
            },
            GuiConfig { strategies: ui },
            self.backend.clone(),
        )
    }

    pub fn join(
        mut process: ProcessConfig<T>,
        backend: BackendConfig,
        mut gui: GuiConfig,
        temperaments: Vec<TemperamentDefinition<T>>,
        named_intervals: Vec<NamedInterval<T>>,
    ) -> Self {
        Self {
            temperaments,
            named_intervals,
            strategies: if process.strategies.len() != gui.strategies.len() {
                panic!(
                    "different number of strategies in the process ({}) and the gui ({})",
                    process.strategies.len(),
                    gui.strategies.len(),
                )
            } else {
                process
                    .strategies
                    .drain(..)
                    .zip(gui.strategies.drain(..))
                    .map(|((strat, _midi_bindings), (names, bindings))| {
                        ExtendedStrategyConfig::join(strat, bindings, names)
                    })
                    .collect()
            },
            backend,
        }
    }
}

#[derive(Clone, Copy)]
pub enum StrategyKind {
    StaticTuning,
}

impl StrategyKind {
    pub fn increment_neighbourhood_index_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
        }
    }
    pub fn set_reference_to_lowest_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
        }
    }
    pub fn set_reference_to_highest_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum NamedCompleteNeighbourhood<T: IntervalBasis> {
    PeriodicComplete {
        name: String,
        #[serde(flatten)]
        inner: PeriodicComplete<T>,
    },
}

impl<T: IntervalBasis> NamedCompleteNeighbourhood<T> {
    fn name(&self) -> &str {
        match self {
            NamedCompleteNeighbourhood::PeriodicComplete { name, .. } => name,
        }
    }

    fn inner(&self) -> SomeCompleteNeighbourhood<T> {
        match self {
            NamedCompleteNeighbourhood::PeriodicComplete { inner, .. } => {
                SomeCompleteNeighbourhood::PeriodicComplete(inner.clone())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedStrategyConfig<T: IntervalBasis> {
    #[serde(rename_all = "kebab-case")]
    StaticTuning {
        name: String,
        description: String,
        bindings: Bindings<Bindable>,
        #[serde(deserialize_with = "deserialize_nonempty_neighbourhoods")]
        neighbourhoods: Vec<NamedCompleteNeighbourhood<T>>,
        tuning_reference: Reference<T>,
        reference: Stack<T>,
    },
}

pub fn deserialize_nonempty_neighbourhoods<
    'de,
    D: serde::Deserializer<'de>,
    T: IntervalBasis + serde::Deserialize<'de>,
>(
    deserializer: D,
) -> Result<Vec<NamedCompleteNeighbourhood<T>>, D::Error> {
    deserialize_nonempty(
        "expected a non-empty list of neighbourhoods for strategy definition",
        deserializer,
    )
}

#[derive(Clone)]
pub struct StrategyNames {
    pub strategy_kind: StrategyKind,
    pub name: String,
    pub description: String,
    pub neighbourhood_names: Vec<String>,
}

impl<T: IntervalBasis> ExtendedStrategyConfig<T> {
    fn split(&self) -> (StrategyConfig<T>, Bindings<Bindable>, StrategyNames) {
        match self {
            ExtendedStrategyConfig::StaticTuning {
                name,
                description,
                bindings,
                neighbourhoods,
                tuning_reference,
                reference,
            } => {
                let neighbourhood_names: Vec<String> =
                    neighbourhoods.iter().map(|x| x.name().into()).collect();
                let neighbourhoods: Vec<SomeCompleteNeighbourhood<T>> =
                    neighbourhoods.iter().map(|x| x.inner()).collect();
                (
                    StrategyConfig::StaticTuning(StaticTuningConfig {
                        neighbourhoods,
                        tuning_reference: tuning_reference.clone(),
                        reference: reference.clone(),
                    }),
                    bindings.clone(),
                    StrategyNames {
                        strategy_kind: StrategyKind::StaticTuning,
                        name: name.clone(),
                        description: description.clone(),
                        neighbourhood_names,
                    },
                )
            }
        }
    }

    // todo make this test something?
    fn join(strat: StrategyConfig<T>, bindings: Bindings<Bindable>, mut names: StrategyNames) -> Self {
        match strat {
            StrategyConfig::StaticTuning(StaticTuningConfig {
                mut neighbourhoods,
                tuning_reference,
                reference,
            }) => Self::StaticTuning {
                name: names.name,
                description: names.description,
                bindings,
                neighbourhoods: if neighbourhoods.len() != names.neighbourhood_names.len() {
                    panic!(
                        "different number of neighbourhoods ({}) and neighbourhood names ({})",
                        neighbourhoods.len(),
                        names.neighbourhood_names.len(),
                    )
                } else {
                    neighbourhoods
                        .drain(..)
                        .zip(names.neighbourhood_names.drain(..))
                        .map(
                            |(SomeCompleteNeighbourhood::PeriodicComplete(inner), name)| {
                                NamedCompleteNeighbourhood::PeriodicComplete { name, inner }
                            },
                        )
                        .collect()
                },
                tuning_reference,
                reference,
            },
        }
    }
}
