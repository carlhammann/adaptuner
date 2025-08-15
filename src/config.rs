use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    bindable::{Bindable, Bindings, MidiBindable},
    custom_serde::common::deserialize_nonempty,
    gui::{
        backend::BackendWindowConfig,
        editor::{reference::ReferenceEditorConfig, tuning::TuningEditorConfig},
        lattice::LatticeWindowConfig,
    },
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, NamedInterval, StackType},
        temperament::TemperamentDefinition,
    },
    neighbourhood::{SomeCompleteNeighbourhood, SomeNeighbourhood},
    reference::Reference,
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::{Strategy, StrategyAction},
        twostep::{
            harmony::chordlist::{keyshape::KeyShape, ChordListConfig, PatternConfig},
            melody::neighbourhoods::NeighbourhoodsConfig,
            TwoStep,
        },
    },
};

pub trait FromConfigAndState<C, S> {
    fn initialise(config: C, state: S) -> Self;
}

pub trait ExtractConfig<C> {
    fn extract_config(&self) -> C;
}

#[derive(Clone)]
pub enum HarmonyStrategyConfig<T: IntervalBasis> {
    ChordList(ChordListConfig<T>),
}

#[derive(Clone)]
pub enum MelodyStrategyConfig<T: IntervalBasis> {
    Neighbourhoods(NeighbourhoodsConfig<T>),
}

#[derive(Clone)]
pub enum StrategyConfig<T: IntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
    TwoStep(HarmonyStrategyConfig<T>, MelodyStrategyConfig<T>),
}

impl<T: StackType> StrategyConfig<T> {
    pub fn realize(self) -> Box<dyn Strategy<T>> {
        match self {
            StrategyConfig::StaticTuning(config) => Box::new(StaticTuning::new(config)),
            StrategyConfig::TwoStep(harmony, melody) => Box::new(TwoStep::new(harmony, melody)),
        }
    }
}

#[derive(Clone)]
pub struct ProcessConfig<T: IntervalBasis> {
    pub strategies: Vec<(StrategyConfig<T>, Bindings<MidiBindable>)>,
}

#[derive(Clone)]
pub struct GuiConfig<T: IntervalBasis> {
    pub strategies: Vec<(StrategyNames<T>, Bindings<Bindable>)>,
    pub lattice_window: LatticeWindowConfig,
    pub backend_window: BackendWindowConfig,
    pub tuning_editor: TuningEditorConfig,
    pub reference_editor: ReferenceEditorConfig,
    pub latency_mean_over: usize,
    pub use_cent_values: bool,
    // pub note_window: NoteWindowConfig<T>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct GuiConfigWithoutStrategies {
    pub lattice_window: LatticeWindowConfig,
    pub tuning_editor: TuningEditorConfig,
    pub reference_editor: ReferenceEditorConfig,
    pub latency_mean_over: usize,
    pub use_cent_values: bool,
    // pub backend_window: BackendWindowConfig, // not necessary since BackendWindowConfig = BackendConfig
    // pub note_window: NoteWindowConfig<T>,
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

/// This (de) serializes as the current version string.
pub struct AdaptunerVersion;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
    version: AdaptunerVersion,
    pub temperaments: Vec<TemperamentDefinition<T>>,
    pub named_intervals: Vec<NamedInterval<T>>,
    strategies: Vec<NamedAndDescribed<ExtendedStrategyConfig<T>>>,
    backend: BackendConfig,
    gui: GuiConfigWithoutStrategies,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct NamedAndDescribed<X> {
    name: String,
    description: String,
    config: X,
}

impl<T: IntervalBasis> Config<T> {
    pub fn split(&self) -> (ProcessConfig<T>, GuiConfig<T>, BackendConfig) {
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
            GuiConfig {
                strategies: ui,
                lattice_window: self.gui.lattice_window.clone(),
                backend_window: self.backend.clone(),
                tuning_editor: self.gui.tuning_editor.clone(),
                reference_editor: self.gui.reference_editor.clone(),
                latency_mean_over: self.gui.latency_mean_over,
                use_cent_values: self.gui.use_cent_values,
                // note_window: self.gui.note_window.clone(),
            },
            self.backend.clone(),
        )
    }

    pub fn join(
        mut process: ProcessConfig<T>,
        backend: BackendConfig,
        mut gui: GuiConfig<T>,
        temperaments: Vec<TemperamentDefinition<T>>,
        named_intervals: Vec<NamedInterval<T>>,
    ) -> Self {
        Self {
            version: AdaptunerVersion,
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
                        <NamedAndDescribed<ExtendedStrategyConfig<T>>>::join(strat, bindings, names)
                    })
                    .collect()
            },
            backend,
            gui: GuiConfigWithoutStrategies {
                lattice_window: gui.lattice_window,
                tuning_editor: gui.tuning_editor,
                reference_editor: gui.reference_editor,
                latency_mean_over: gui.latency_mean_over,
                use_cent_values: gui.use_cent_values,
            },
        }
    }
}

#[derive(Clone, Copy)]
pub enum HarmonyStrategyKind {
    ChordList,
}

#[derive(Clone, Copy)]
pub enum MelodyStrategyKind {
    Neighbourhoods,
}

#[derive(Clone, Copy)]
pub enum StrategyKind {
    StaticTuning,
    TwoStep(HarmonyStrategyKind, MelodyStrategyKind),
}

impl StrategyKind {
    /// For parameterised actions like [StrategyAction::IncrementNeighbourhoodIndex], if it returns
    /// true for one parameter, it will return true for all parameters.
    pub fn action_allowed(&self, action: &StrategyAction) -> bool {
        match (self, action) {
            (_, StrategyAction::Reset) => true,
            (StrategyKind::StaticTuning, StrategyAction::IncrementNeighbourhoodIndex(_)) => true,
            (StrategyKind::StaticTuning, StrategyAction::SetReferenceToLowest) => true,
            (StrategyKind::StaticTuning, StrategyAction::SetReferenceToHighest) => true,
            (StrategyKind::StaticTuning, StrategyAction::SetReferenceToCurrent) => false,
            (StrategyKind::StaticTuning, StrategyAction::ToggleChordMatching) => false,
            (StrategyKind::StaticTuning, StrategyAction::ToggleReanchor) => false,
            (
                StrategyKind::TwoStep(_, MelodyStrategyKind::Neighbourhoods),
                StrategyAction::IncrementNeighbourhoodIndex(_),
            ) => true,
            (
                StrategyKind::TwoStep(_, MelodyStrategyKind::Neighbourhoods),
                StrategyAction::SetReferenceToLowest,
            ) => false,
            (
                StrategyKind::TwoStep(_, MelodyStrategyKind::Neighbourhoods),
                StrategyAction::SetReferenceToHighest,
            ) => false,
            (
                StrategyKind::TwoStep(_, MelodyStrategyKind::Neighbourhoods),
                StrategyAction::SetReferenceToCurrent,
            ) => true,
            (
                StrategyKind::TwoStep(_, MelodyStrategyKind::Neighbourhoods),
                StrategyAction::ToggleReanchor,
            ) => true,
            (
                StrategyKind::TwoStep(HarmonyStrategyKind::ChordList, _),
                StrategyAction::ToggleChordMatching,
            ) => true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct NamedCompleteNeighbourhood<T: IntervalBasis> {
    name: String,
    entries: SomeCompleteNeighbourhood<T>,
}

impl<T: IntervalBasis> NamedCompleteNeighbourhood<T> {
    fn name(&self) -> &str {
        match self {
            NamedCompleteNeighbourhood { name, .. } => name,
        }
    }

    fn inner(&self) -> SomeCompleteNeighbourhood<T> {
        match self {
            NamedCompleteNeighbourhood { entries: inner, .. } => inner.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ExtendedStaticTuningConfig<T: IntervalBasis> {
    #[serde(deserialize_with = "deserialize_nonempty_neighbourhoods")]
    neighbourhoods: Vec<NamedCompleteNeighbourhood<T>>,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
    bindings: Bindings<Bindable>,
}

fn deserialize_nonempty_neighbourhoods<
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedHarmonyStrategyConfig<T: IntervalBasis> {
    ChordList {
        enable: bool,
        patterns: Vec<NamedPatternConfig<T>>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedMelodyStrategyConfig<T: IntervalBasis> {
    Neighbourhoods(ExtendedNeighbourhoodsConfig<T>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ExtendedNeighbourhoodsConfig<T: IntervalBasis> {
    fixed: bool,
    group_ms: u64,
    #[serde(deserialize_with = "deserialize_nonempty_neighbourhoods")]
    neighbourhoods: Vec<NamedCompleteNeighbourhood<T>>,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedStrategyConfig<T: IntervalBasis> {
    StaticTuning(ExtendedStaticTuningConfig<T>),
    TwoStep {
        harmony: ExtendedHarmonyStrategyConfig<T>,
        melody: ExtendedMelodyStrategyConfig<T>,
        bindings: Bindings<Bindable>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct NamedPatternConfig<T: IntervalBasis> {
    pub name: String,
    pub key_shape: KeyShape,
    pub neighbourhood: SomeNeighbourhood<T>,
    pub allow_extra_high_notes: bool,
    pub original_reference: Stack<T>,
}

#[derive(Clone)]
pub enum HarmonyStrategyNames<T: IntervalBasis> {
    ChordList {
        patterns: Vec<NamedPatternConfig<T>>,
    },
}

#[derive(Clone)]
pub enum MelodyStrategyNames {
    Neighbourhoods {
        group_ms: u64,
        fixed: bool,
        neighbourhood_names: Vec<String>,
    },
}

#[derive(Clone)]
pub enum StrategyNames<T: IntervalBasis> {
    StaticTuning {
        name: String,
        description: String,
        neighbourhood_names: Vec<String>,
    },
    TwoStep {
        name: String,
        description: String,
        harmony: HarmonyStrategyNames<T>,
        melody: MelodyStrategyNames,
    },
}

impl<T: IntervalBasis> StrategyNames<T> {
    pub fn strategy_kind(&self) -> StrategyKind {
        match self {
            StrategyNames::StaticTuning { .. } => StrategyKind::StaticTuning,
            StrategyNames::TwoStep {
                harmony: HarmonyStrategyNames::ChordList { .. },
                melody: MelodyStrategyNames::Neighbourhoods { .. },
                ..
            } => StrategyKind::TwoStep(
                HarmonyStrategyKind::ChordList,
                MelodyStrategyKind::Neighbourhoods,
            ),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            StrategyNames::StaticTuning { name, .. } => name,
            StrategyNames::TwoStep { name, .. } => name,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            StrategyNames::StaticTuning { name, .. } => name,
            StrategyNames::TwoStep { name, .. } => name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            StrategyNames::StaticTuning { description, .. } => description,
            StrategyNames::TwoStep { description, .. } => description,
        }
    }

    pub fn description_mut(&mut self) -> &mut String {
        match self {
            StrategyNames::StaticTuning { description, .. } => description,
            StrategyNames::TwoStep { description, .. } => description,
        }
    }

    pub fn neighbourhood_names_mut(&mut self) -> &mut Vec<String> {
        match self {
            StrategyNames::StaticTuning {
                neighbourhood_names,
                ..
            } => neighbourhood_names,
            StrategyNames::TwoStep {
                melody:
                    MelodyStrategyNames::Neighbourhoods {
                        neighbourhood_names,
                        ..
                    },
                ..
            } => neighbourhood_names,
        }
    }
}

impl<T: IntervalBasis> ExtendedStaticTuningConfig<T> {
    fn split(&self) -> (StaticTuningConfig<T>, Bindings<Bindable>, Vec<String>) {
        let ExtendedStaticTuningConfig {
            bindings,
            neighbourhoods,
            tuning_reference,
            reference,
        } = self;

        let neighbourhood_names: Vec<String> =
            neighbourhoods.iter().map(|x| x.name().into()).collect();
        let neighbourhoods: Vec<SomeCompleteNeighbourhood<T>> =
            neighbourhoods.iter().map(|x| x.inner()).collect();
        (
            StaticTuningConfig {
                neighbourhoods,
                tuning_reference: tuning_reference.clone(),
                reference: reference.clone(),
            },
            bindings.clone(),
            neighbourhood_names,
        )
    }

    fn join(
        strat: StaticTuningConfig<T>,
        bindings: Bindings<Bindable>,
        mut neighbourhood_names: Vec<String>,
    ) -> Self {
        let StaticTuningConfig {
            mut neighbourhoods,
            tuning_reference,
            reference,
        } = strat;

        ExtendedStaticTuningConfig {
            bindings,
            neighbourhoods: if neighbourhoods.len() != neighbourhood_names.len() {
                panic!(
                    "different number of neighbourhoods ({}) and neighbourhood names ({})",
                    neighbourhoods.len(),
                    neighbourhood_names.len(),
                )
            } else {
                neighbourhoods
                    .drain(..)
                    .zip(neighbourhood_names.drain(..))
                    .map(|(inner, name)| NamedCompleteNeighbourhood {
                        name,
                        entries: inner,
                    })
                    .collect()
            },
            tuning_reference,
            reference,
        }
    }
}

impl<T: IntervalBasis> ExtendedHarmonyStrategyConfig<T> {
    fn split(&self) -> (HarmonyStrategyConfig<T>, HarmonyStrategyNames<T>) {
        match self {
            ExtendedHarmonyStrategyConfig::ChordList { enable, patterns } => (
                HarmonyStrategyConfig::ChordList(ChordListConfig {
                    enable: *enable,
                    patterns: patterns
                        .iter()
                        .map(|p| PatternConfig {
                            key_shape: p.key_shape.clone(),
                            neighbourhood: p.neighbourhood.clone(),
                            allow_extra_high_notes: p.allow_extra_high_notes,
                        })
                        .collect(),
                }),
                HarmonyStrategyNames::ChordList {
                    patterns: patterns.clone(),
                },
            ),
        }
    }

    fn join(strat: HarmonyStrategyConfig<T>, names: HarmonyStrategyNames<T>) -> Self {
        match (strat, names) {
            (
                HarmonyStrategyConfig::ChordList(ChordListConfig { enable, patterns }),
                HarmonyStrategyNames::ChordList {
                    patterns: named_patterns,
                },
            ) => {
                if patterns.len() != named_patterns.len() {
                    panic!("different numbers of patterns in the chord list and names for these patterns");
                }
                ExtendedHarmonyStrategyConfig::ChordList {
                    enable,
                    patterns: named_patterns,
                }
            }
        }
    }
}

impl<T: IntervalBasis> ExtendedMelodyStrategyConfig<T> {
    fn split(&self) -> (MelodyStrategyConfig<T>, MelodyStrategyNames) {
        match self {
            ExtendedMelodyStrategyConfig::Neighbourhoods(ExtendedNeighbourhoodsConfig {
                fixed,
                group_ms,
                neighbourhoods,
                tuning_reference,
                reference,
            }) => {
                let neighbourhood_names: Vec<String> =
                    neighbourhoods.iter().map(|x| x.name().into()).collect();
                let neighbourhoods: Vec<SomeCompleteNeighbourhood<T>> =
                    neighbourhoods.iter().map(|x| x.inner()).collect();
                (
                    MelodyStrategyConfig::Neighbourhoods(NeighbourhoodsConfig {
                        fixed: *fixed,
                        group_ms: *group_ms,
                        inner: StaticTuningConfig {
                            neighbourhoods,
                            tuning_reference: tuning_reference.clone(),
                            reference: reference.clone(),
                        },
                    }),
                    MelodyStrategyNames::Neighbourhoods {
                        neighbourhood_names,
                        group_ms: *group_ms,
                        fixed: *fixed,
                    },
                )
            }
        }
    }

    fn join(strat: MelodyStrategyConfig<T>, names: MelodyStrategyNames) -> Self {
        match (strat, names) {
            (
                MelodyStrategyConfig::Neighbourhoods(NeighbourhoodsConfig {
                    fixed,
                    group_ms,
                    inner:
                        StaticTuningConfig {
                            mut neighbourhoods,
                            tuning_reference,
                            reference,
                        },
                }),
                MelodyStrategyNames::Neighbourhoods {
                    mut neighbourhood_names,
                    .. //group_ms, fixed
                },
            ) => Self::Neighbourhoods(ExtendedNeighbourhoodsConfig {
                fixed,
                group_ms,
                neighbourhoods: if neighbourhoods.len() != neighbourhood_names.len() {
                    panic!(
                        "different number of neighbourhoods ({}) and neighbourhood names ({})",
                        neighbourhoods.len(),
                        neighbourhood_names.len(),
                    )
                } else {
                    neighbourhoods
                        .drain(..)
                        .zip(neighbourhood_names.drain(..))
                        .map(|(inner, name)| NamedCompleteNeighbourhood {
                            name,
                            entries: inner,
                        })
                        .collect()
                },
                tuning_reference,
                reference,
            }),
        }
    }
}

impl<T: IntervalBasis> NamedAndDescribed<ExtendedStrategyConfig<T>> {
    fn split(&self) -> (StrategyConfig<T>, Bindings<Bindable>, StrategyNames<T>) {
        match self {
            NamedAndDescribed {
                name,
                description,
                config: ExtendedStrategyConfig::StaticTuning(inner),
            } => {
                let (c, b, neighbourhood_names) = inner.split();
                (
                    StrategyConfig::StaticTuning(c),
                    b,
                    StrategyNames::StaticTuning {
                        name: name.clone(),
                        description: description.clone(),
                        neighbourhood_names,
                    },
                )
            }
            NamedAndDescribed {
                name,
                description,
                config:
                    ExtendedStrategyConfig::TwoStep {
                        bindings,
                        harmony,
                        melody,
                    },
            } => {
                let (harmony_config, harmony_names) = harmony.split();
                let (melody_config, melody_names) = melody.split();
                (
                    StrategyConfig::TwoStep(harmony_config, melody_config),
                    bindings.clone(),
                    StrategyNames::TwoStep {
                        name: name.clone(),
                        description: description.clone(),
                        harmony: harmony_names,
                        melody: melody_names,
                    },
                )
            }
        }
    }

    fn join(
        strat: StrategyConfig<T>,
        bindings: Bindings<Bindable>,
        names: StrategyNames<T>,
    ) -> Self {
        match (strat, names) {
            (
                StrategyConfig::StaticTuning(c),
                StrategyNames::StaticTuning {
                    name,
                    description,
                    neighbourhood_names,
                },
            ) => Self {
                name,
                description,
                config: ExtendedStrategyConfig::StaticTuning(ExtendedStaticTuningConfig::join(
                    c,
                    bindings,
                    neighbourhood_names,
                )),
            },
            (
                StrategyConfig::TwoStep(harmony_config, melody_config),
                StrategyNames::TwoStep {
                    name,
                    description,
                    harmony: harmony_names,
                    melody: melody_names,
                },
            ) => Self {
                name,
                description,
                config: ExtendedStrategyConfig::TwoStep {
                    bindings,
                    harmony: ExtendedHarmonyStrategyConfig::join(harmony_config, harmony_names),
                    melody: ExtendedMelodyStrategyConfig::join(melody_config, melody_names),
                },
            },
            _ => panic!("strategy config and strategy names don't have matching types"),
        }
    }
}
