use std::fmt;

use serde_derive::{Deserialize, Serialize};

use crate::{
    interval::{
        stacktype::r#trait::{PeriodicIntervalBasis, PeriodicStackType},
        temperament::TemperamentDefinition,
    },
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::Strategy,
    },
};

// PeriodicIntervalBasis or PeriodicStackType is required for (De-)Serialize of neighbourhoods (for now...)

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config<T: PeriodicIntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub strategies: Vec<StrategyConfig<T>>,
}

pub enum StrategyKind {
    Static,
}

impl<T: PeriodicIntervalBasis> Config<T> {
    pub fn strategy_names_and_kinds(&self) -> Vec<(String, StrategyKind)> {
        self.strategies
            .iter()
            .map(|conf| match conf {
                StrategyConfig::StaticTuning { name, .. } => (name.clone(), StrategyKind::Static),
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyConfig<T: PeriodicIntervalBasis> {
    StaticTuning {
        name: String,
        #[serde(flatten)]
        config: StaticTuningConfig<T>,
    },
}

pub fn initialise_strategy<T>(conf: StrategyConfig<T>) -> Box<dyn Strategy<T>>
where
    T: PeriodicStackType + fmt::Debug + 'static,
{
    match conf {
        StrategyConfig::StaticTuning { config, .. } => Box::new(StaticTuning::new(config)),
    }
}
