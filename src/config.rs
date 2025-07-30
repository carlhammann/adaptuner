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
    neighbourhood::{PeriodicComplete, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::{
        r#static::{StaticTuning, StaticTuningConfig},
        r#trait::Strategy,
        twostep::{harmony::chordlist::ChordListConfig, TwoStep},
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
pub enum HarmonyStrategyConfig<T: IntervalBasis> {
    ChordList(ChordListConfig<T>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum MelodyStrategyConfig<T: IntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
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
pub struct GuiConfig {
    pub strategies: Vec<(StrategyNames, Bindings<Bindable>)>,
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config<T: IntervalBasis> {
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
    StaticTuning,
}

#[derive(Clone, Copy)]
pub enum StrategyKind {
    StaticTuning,
    TwoStep(HarmonyStrategyKind, MelodyStrategyKind),
}

impl StrategyKind {
    pub fn increment_neighbourhood_index_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
            StrategyKind::TwoStep(_, MelodyStrategyKind::StaticTuning) => true,
        }
    }
    pub fn set_reference_to_lowest_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
            StrategyKind::TwoStep(_, MelodyStrategyKind::StaticTuning) => true,
        }
    }
    pub fn set_reference_to_highest_allowed(&self) -> bool {
        match self {
            StrategyKind::StaticTuning => true,
            StrategyKind::TwoStep(_, MelodyStrategyKind::StaticTuning) => true,
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
pub struct ExtendedStaticTuningConfig<T: IntervalBasis> {
    #[serde(deserialize_with = "deserialize_nonempty_neighbourhoods")]
    neighbourhoods: Vec<NamedCompleteNeighbourhood<T>>,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
    bindings: Bindings<Bindable>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedHarmonyStrategyConfig<T: IntervalBasis> {
    ChordList(ChordListConfig<T>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedMelodyStrategyConfig<T: IntervalBasis> {
    StaticTuning(ExtendedStaticTuningConfig<T>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum ExtendedStrategyConfig<T: IntervalBasis> {
    StaticTuning(ExtendedStaticTuningConfig<T>),
    TwoStep {
        harmony: ExtendedHarmonyStrategyConfig<T>,
        melody: ExtendedMelodyStrategyConfig<T>,
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
pub enum HarmonyStrategyNames {
    ChordList,
}

#[derive(Clone)]
pub enum MelodyStrategyNames {
    StaticTuning { neighbourhood_names: Vec<String> },
}

#[derive(Clone)]
pub enum StrategyNames {
    StaticTuning {
        name: String,
        description: String,
        neighbourhood_names: Vec<String>,
    },
    TwoStep {
        name: String,
        description: String,
        harmony: HarmonyStrategyNames,
        melody: MelodyStrategyNames,
    },
}

impl StrategyNames {
    pub fn strategy_kind(&self) -> StrategyKind {
        match self {
            StrategyNames::StaticTuning { .. } => StrategyKind::StaticTuning,
            StrategyNames::TwoStep {
                harmony: HarmonyStrategyNames::ChordList,
                melody: MelodyStrategyNames::StaticTuning { .. },
                ..
            } => StrategyKind::TwoStep(
                HarmonyStrategyKind::ChordList,
                MelodyStrategyKind::StaticTuning,
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
                    MelodyStrategyNames::StaticTuning {
                        neighbourhood_names,
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
                    .map(
                        |(SomeCompleteNeighbourhood::PeriodicComplete(inner), name)| {
                            NamedCompleteNeighbourhood::PeriodicComplete { name, inner }
                        },
                    )
                    .collect()
            },
            tuning_reference,
            reference,
        }
    }
}

impl<T: IntervalBasis> ExtendedHarmonyStrategyConfig<T> {
    fn split(&self) -> (HarmonyStrategyConfig<T>, HarmonyStrategyNames) {
        match self {
            ExtendedHarmonyStrategyConfig::ChordList(c) => (
                HarmonyStrategyConfig::ChordList(c.clone()),
                HarmonyStrategyNames::ChordList,
            ),
        }
    }

    fn join(strat: HarmonyStrategyConfig<T>, _names: HarmonyStrategyNames) -> Self {
        match strat {
            HarmonyStrategyConfig::ChordList(c) => ExtendedHarmonyStrategyConfig::ChordList(c),
        }
    }
}

impl<T: IntervalBasis> ExtendedMelodyStrategyConfig<T> {
    fn split(
        &self,
    ) -> (
        MelodyStrategyConfig<T>,
        Bindings<Bindable>,
        MelodyStrategyNames,
    ) {
        match self {
            ExtendedMelodyStrategyConfig::StaticTuning(ExtendedStaticTuningConfig {
                bindings,
                neighbourhoods,
                tuning_reference,
                reference,
            }) => {
                let neighbourhood_names: Vec<String> =
                    neighbourhoods.iter().map(|x| x.name().into()).collect();
                let neighbourhoods: Vec<SomeCompleteNeighbourhood<T>> =
                    neighbourhoods.iter().map(|x| x.inner()).collect();
                (
                    MelodyStrategyConfig::StaticTuning(StaticTuningConfig {
                        neighbourhoods,
                        tuning_reference: tuning_reference.clone(),
                        reference: reference.clone(),
                    }),
                    bindings.clone(),
                    MelodyStrategyNames::StaticTuning {
                        neighbourhood_names,
                    },
                )
            }
        }
    }

    fn join(
        strat: MelodyStrategyConfig<T>,
        bindings: Bindings<Bindable>,
        names: MelodyStrategyNames,
    ) -> Self {
        match (strat, names) {
            (
                MelodyStrategyConfig::StaticTuning(StaticTuningConfig {
                    mut neighbourhoods,
                    tuning_reference,
                    reference,
                }),
                MelodyStrategyNames::StaticTuning {
                    mut neighbourhood_names,
                },
            ) => Self::StaticTuning(ExtendedStaticTuningConfig {
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
                        .map(
                            |(SomeCompleteNeighbourhood::PeriodicComplete(inner), name)| {
                                NamedCompleteNeighbourhood::PeriodicComplete { name, inner }
                            },
                        )
                        .collect()
                },
                tuning_reference,
                reference,
            }),
        }
    }
}

impl<T: IntervalBasis> NamedAndDescribed<ExtendedStrategyConfig<T>> {
    fn split(&self) -> (StrategyConfig<T>, Bindings<Bindable>, StrategyNames) {
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
                config: ExtendedStrategyConfig::TwoStep { harmony, melody },
            } => {
                let (harmony_config, harmony_names) = harmony.split();
                let (melody_config, bindings, melody_names) = melody.split();
                (
                    StrategyConfig::TwoStep(harmony_config, melody_config),
                    bindings,
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

    fn join(strat: StrategyConfig<T>, bindings: Bindings<Bindable>, names: StrategyNames) -> Self {
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
                    harmony: ExtendedHarmonyStrategyConfig::join(harmony_config, harmony_names),
                    melody: ExtendedMelodyStrategyConfig::join(
                        melody_config,
                        bindings,
                        melody_names,
                    ),
                },
            },
            _ => panic!("strategy config and strategy names don't have matching types"),
        }
    }
}
