use std::{fmt, sync::LazyLock};

use serde_derive::{Deserialize, Serialize};

use crate::{
    bindable::{Bindings, OneBinding},
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
        r#trait::{Strategy, StrategyAction},
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

impl StrategyKind {
    pub fn allowed_actions(&self) -> &'static [StrategyAction] {
        match self {
            StrategyKind::StaticTuning => &[
                StrategyAction::NextNeighbourhood,
                StrategyAction::PrevNeighbourhood,
                StrategyAction::SetReferenceToLowest,
                StrategyAction::SetReferenceToHighest,
            ],
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
pub struct ExtendedStrategyConfig<T: IntervalBasis> {
    pub name: String,
    pub description: String,
    pub config: StrategyConfig<T>,
    pub bindings: Bindings<StrategyAction>,
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

pub static STRATEGY_TEMPLATES: LazyLock<[ExtendedStrategyConfig<TheFiveLimitStackType>; 1]> =
    LazyLock::new(|| {
        [ExtendedStrategyConfig {
            name: "static".into(),
            description: r#"This strategy allows you to
• define the (static) tuning of all 12 notes as a "neighbourhood" of the reference note,
• switch between different neighbourhoods on the fly, and
• reset the reference note on the fly."#
                .into(),
            config: StrategyConfig::StaticTuning(StaticTuningConfig {
                neighbourhoods: vec![PeriodicComplete::from_octave_tunings(
                    "just intonation".into(),
                    [
                        Stack::new_zero(),                  // C
                        Stack::from_target(vec![0, -1, 2]), // C#
                        Stack::from_target(vec![-1, 2, 0]), // D
                        Stack::from_target(vec![0, 1, -1]), // Eb
                        Stack::from_target(vec![0, 0, 1]),  // E
                        Stack::from_target(vec![1, -1, 0]), // F
                        Stack::from_target(vec![-1, 2, 1]), // F#+
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
            bindings: Bindings {
                soft_pedal: OneBinding {
                    on_down: Some(StrategyAction::SetReferenceToLowest),
                    on_up: None {},
                },
                sostenuto_pedal: OneBinding {
                    on_down: Some(StrategyAction::NextNeighbourhood),
                    on_up: Some(StrategyAction::PrevNeighbourhood),
                },
            },
        }]
    });
