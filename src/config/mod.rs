use std::marker::PhantomData;

use midi_msg::Channel;
use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::{pitchbend::*, r#trait::BackendState},
    config::r#trait::Config,
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{PeriodicStackType, StackCoeff, StackType},
        },
    },
    neighbourhood,
    neighbourhood::{new_fivelimit_neighbourhood, PeriodicPartial, SomeNeighbourhood},
    notename::NoteNameStyle,
    pattern::{KeyShape, Pattern},
    process::{
        r#trait::ProcessState,
        walking::{Walking, WalkingConfig},
    },
    tui::{
        grid::{DisplayConfig, GridConfig},
        latencyreporter::LatencyReporterConfig,
        onlynotify::{OnlyNotify, OnlyNotifyConfig},
        r#trait::UIState,
        wrappedgrid::{WrappedGrid, WrappedGridConfig},
    },
};

pub mod r#trait;

#[derive(Clone)]
pub enum MidiPortConfig {
    AskAtStartup,
}

pub struct CompleteConfig<T, P, PCONFIG, B, BCONFIG, U, UCONFIG>
where
    T: StackType,
    P: ProcessState<T>,
    PCONFIG: Config<P>,
    B: BackendState<T>,
    BCONFIG: Config<B>,
    U: UIState<T>,
    UCONFIG: Config<U>,
{
    pub midi_port_config: MidiPortConfig,
    pub process_config: PCONFIG,
    pub backend_config: BCONFIG,
    pub ui_config: UCONFIG,
    pub _phantom: PhantomData<(T, P, B, U)>,
}

#[derive(Serialize, Deserialize)]
pub struct SimplePatternConfig {
    name: String,
    keyshape: KeyShape,
    neighbourhood: Vec<(usize, [StackCoeff; 3])>,
}

impl From<SimplePatternConfig> for Pattern<ConcreteFiveLimitStackType> {
    fn from(simple: SimplePatternConfig) -> Self {
        let octave = Stack::from_pure_interval(ConcreteFiveLimitStackType::period_index());
        let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
        Pattern {
            name: simple.name,
            keyshape: simple.keyshape,
            neighbourhood: SomeNeighbourhood::PeriodicPartial(PeriodicPartial {
                stacks: simple
                    .neighbourhood
                    .into_iter()
                    .map(|(offset, coeffs)| {
                        (offset, Stack::new(&no_active_temperaments, coeffs.to_vec()))
                    })
                    .collect(),
                period: octave,
            }),
        }
    }
}

/// See the restrictions on [new_fivelimit_neighbourhood] on the first three arguments!
pub fn init_walking_config(
    initial_neighbourhood_width: StackCoeff,
    initial_neighbourhood_index: StackCoeff,
    initial_neighbourhood_offset: StackCoeff,
    patterns: Vec<SimplePatternConfig>,
) -> CompleteConfig<
    ConcreteFiveLimitStackType,
    Walking<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
    WalkingConfig<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
    Pitchbend<15>,
    PitchbendConfig<15>,
    WrappedGrid<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
    WrappedGridConfig<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
> {
    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
    let initial_neighbourhood = new_fivelimit_neighbourhood(
        &no_active_temperaments,
        initial_neighbourhood_width,
        initial_neighbourhood_index,
        initial_neighbourhood_offset,
    );
    CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: WalkingConfig {
            _phantom: PhantomData,
            temper_pattern_neighbourhoods: false,
            initial_neighbourhood: initial_neighbourhood.clone(),
            patterns: patterns.into_iter().map(From::from).collect(),
            consider_played: false,
        },
        backend_config: PitchbendConfig {
            channels: [
                Channel::Ch1,
                Channel::Ch2,
                Channel::Ch3,
                Channel::Ch4,
                Channel::Ch5,
                Channel::Ch6,
                Channel::Ch7,
                Channel::Ch8,
                Channel::Ch9,
                Channel::Ch11,
                Channel::Ch12,
                Channel::Ch13,
                Channel::Ch14,
                Channel::Ch15,
                Channel::Ch16,
            ],
            bend_range: 2.0,
        },
        ui_config: WrappedGridConfig {
            gridconfig: GridConfig {
                display_config: DisplayConfig {
                    notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
                    color_range: 0.2,
                    gradient: colorous::RED_BLUE,
                },
                initial_reference_key: 60,
                initial_neighbourhood,
                horizontal_index: 1,
                vertical_index: 2,
                fifth_index: 1,
                third_index: 2,
                _phantom: PhantomData,
            },
            latencyreporterconfig: LatencyReporterConfig { nsamples: 20 },
        },
        _phantom: PhantomData,
    }
}

/// See the restrictions on [new_fivelimit_neighbourhood] on the first three arguments!
pub fn init_walking_debug_config(
    initial_neighbourhood_width: StackCoeff,
    initial_neighbourhood_index: StackCoeff,
    initial_neighbourhood_offset: StackCoeff,
    patterns: Vec<SimplePatternConfig>,
) -> CompleteConfig<
    ConcreteFiveLimitStackType,
    Walking<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
    WalkingConfig<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
    >,
    Pitchbend<15>,
    PitchbendConfig<15>,
    OnlyNotify,
    OnlyNotifyConfig,
> {
    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
    let initial_neighbourhood = new_fivelimit_neighbourhood(
        &no_active_temperaments,
        initial_neighbourhood_width,
        initial_neighbourhood_index,
        initial_neighbourhood_offset,
    );
    CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: WalkingConfig {
            _phantom: PhantomData,
            temper_pattern_neighbourhoods: false,
            initial_neighbourhood: initial_neighbourhood.clone(),
            patterns: patterns.into_iter().map(From::from).collect(),
            consider_played: false,
        },
        backend_config: PitchbendConfig {
            channels: [
                Channel::Ch1,
                Channel::Ch2,
                Channel::Ch3,
                Channel::Ch4,
                Channel::Ch5,
                Channel::Ch6,
                Channel::Ch7,
                Channel::Ch8,
                Channel::Ch9,
                Channel::Ch11,
                Channel::Ch12,
                Channel::Ch13,
                Channel::Ch14,
                Channel::Ch15,
                Channel::Ch16,
            ],
            bend_range: 2.0,
        },
        ui_config: OnlyNotifyConfig {},
        _phantom: PhantomData,
    }
}
