use std::{fmt, sync::LazyLock};

use serde_derive::{Deserialize, Serialize};

use crate::{
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::TheFiveLimitStackType,
            r#trait::{IntervalBasis, StackType},
        },
        temperament::TemperamentDefinition,
    },
    neighbourhood::PeriodicComplete,
    reference::Reference,
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::Strategy,
    },
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config<T: IntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub strategies: Vec<ExtendedStrategyConfig<T>>,
}

#[derive(Clone, Copy)]
pub enum StrategyKind {
    StaticTuning,
}

impl<T: IntervalBasis> Config<T> {
    pub fn strategy_names_and_kinds(&self) -> Vec<(String, StrategyKind)> {
        self.strategies
            .iter()
            .map(|conf| (conf.name.clone(), conf.strategy_kind()))
            .collect()
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
pub struct ExtendedStrategyConfig<T: IntervalBasis> {
    pub name: String,
    pub config: StrategyConfig<T>,
}

impl<T: IntervalBasis> ExtendedStrategyConfig<T> {
    pub fn strategy_kind(&self) -> StrategyKind {
        match self {
            ExtendedStrategyConfig {
                config: StrategyConfig::StaticTuning(_),
                ..
            } => StrategyKind::StaticTuning,
        }
    }
}

impl<T: StackType + fmt::Debug + 'static> ExtendedStrategyConfig<T> {
    pub fn realize(self) -> Box<dyn Strategy<T>> {
        self.config.realize()
    }
}

pub static STRATEGY_TEMPLATES: LazyLock<[ExtendedStrategyConfig<TheFiveLimitStackType>; 1]> =
    LazyLock::new(|| {
        [ExtendedStrategyConfig {
            name: "static tuning".into(),
            config: StrategyConfig::StaticTuning(StaticTuningConfig {
                neighbourhoods: vec![PeriodicComplete::from_octave_tunings(
                    "JI".into(),
                    [
                        Stack::new_zero(),                  // C
                        Stack::from_target(vec![0, -1, 2]), // C#
                        Stack::from_target(vec![-1, 2, 0]), // D
                        Stack::from_target(vec![0, 1, -1]), // Eb
                        Stack::from_target(vec![0, 0, 1]),  // E
                        Stack::from_target(vec![1, -1, 0]), // F
                        Stack::from_target(vec![-1, 2, 1]), // F3+
                        Stack::from_target(vec![0, 1, 0]),  // G
                        Stack::from_target(vec![0, 0, 2]),  // G#
                        Stack::from_target(vec![1, -1, 1]), // A
                        Stack::from_target(vec![0, 2, -1]), // Bb
                        Stack::from_target(vec![0, 1, 1]),  // B
                    ],
                )
                .into()],
                tuning_reference: Reference {
                    stack: Stack::new_zero(),
                    semitones: 60.0,
                },
                reference: Stack::new_zero(),
            }),
        }]
    });
