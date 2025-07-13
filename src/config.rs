use serde_derive::{Deserialize, Serialize};

use crate::{
    interval::{stacktype::r#trait::PeriodicIntervalBasis, temperament::TemperamentDefinition},
    strategy::r#static::StaticTuningConfig,
};

// PeriodicIntervalBasis is required for (De-)Serialize of neighbourhoods (for now...)

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config<T: PeriodicIntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub strategies: Vec<StrategyConfig<T>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyConfig<T: PeriodicIntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
}
