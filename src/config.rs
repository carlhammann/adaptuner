use std::{collections::BTreeMap, fmt};

use serde_derive::{Deserialize, Serialize};

use crate::{
    bindable::Bindable,
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
        r#trait::{Strategy, StrategyAction},
    },
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub named_intervals: Vec<NamedInterval<T>>,
    pub strategies: Vec<ExtendedStrategyConfig<T>>,
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
pub enum StrategyConfig<T: IntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
}

impl<T: StackType + fmt::Debug + 'static> StrategyConfig<T> {
    pub fn realize(self) -> Box<dyn Strategy<T>> {
        match self {
            StrategyConfig::StaticTuning(config) => Box::new(StaticTuning::new(config)),
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

    fn inner(self) -> SomeCompleteNeighbourhood<T> {
        match self {
            NamedCompleteNeighbourhood::PeriodicComplete { inner, .. } => {
                SomeCompleteNeighbourhood::PeriodicComplete(inner)
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
        bindings: BTreeMap<Bindable, StrategyAction>,
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
pub struct StrategyNamesAndBindings {
    pub strategy_kind: StrategyKind,
    pub name: String,
    pub description: String,
    pub neighbourhood_names: Vec<String>,
    pub bindings: BTreeMap<Bindable, StrategyAction>,
}

impl<T: IntervalBasis> ExtendedStrategyConfig<T> {
    pub fn strategy_kind(&self) -> StrategyKind {
        match self {
            ExtendedStrategyConfig::StaticTuning { .. } => StrategyKind::StaticTuning,
        }
    }

    pub fn split(
        self,
    ) -> (
        (StrategyConfig<T>, BTreeMap<Bindable, StrategyAction>),
        StrategyNamesAndBindings,
    ) {
        match self {
            ExtendedStrategyConfig::StaticTuning {
                name,
                description,
                bindings,
                mut neighbourhoods,
                tuning_reference,
                reference,
            } => {
                let neighbourhood_names: Vec<String> =
                    neighbourhoods.iter().map(|x| x.name().into()).collect();
                let neighbourhoods: Vec<SomeCompleteNeighbourhood<T>> =
                    neighbourhoods.drain(..).map(|x| x.inner()).collect();
                (
                    (
                        StrategyConfig::StaticTuning(StaticTuningConfig {
                            neighbourhoods,
                            tuning_reference,
                            reference,
                        }),
                        bindings.clone(),
                    ),
                    StrategyNamesAndBindings {
                        strategy_kind: StrategyKind::StaticTuning,
                        name,
                        description,
                        neighbourhood_names,
                        bindings,
                    },
                )
            }
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ExtendedStrategyConfig::StaticTuning { name, .. } => name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            ExtendedStrategyConfig::StaticTuning { description, .. } => description,
        }
    }

    pub fn bindings(&self) -> &BTreeMap<Bindable, StrategyAction> {
        match self {
            ExtendedStrategyConfig::StaticTuning { bindings, .. } => bindings,
        }
    }

    pub fn bindings_mut(&mut self) -> &mut BTreeMap<Bindable, StrategyAction> {
        match self {
            ExtendedStrategyConfig::StaticTuning { bindings, .. } => bindings,
        }
    }
}
