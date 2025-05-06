use std::marker::PhantomData;

use midi_msg::Channel;

use crate::{
    backend::{
        pitchbend12::{Pitchbend12, Pitchbend12Config},
        r#trait::BackendState,
    },
    config::r#trait::Config,
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{StackCoeff, StackType},
        },
    },
    neighbourhood::{self, new_fivelimit_neighbourhood, PeriodicCompleteAligned},
    notename::NoteNameStyle,
    process::{
        fromstrategy,
        r#trait::ProcessState,
        //walking::{Walking, WalkingConfig},
    },
    reference::Reference,
    strategy::r#static,
    tui::{
        self,
        grid::{DisplayConfig, Grid, GridConfig},
        latencyreporter::{self, LatencyReporter, LatencyReporterConfig},
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

//#[derive(Serialize, Deserialize)]
//pub struct SimplePatternConfig {
//    name: String,
//    keyshape: KeyShape,
//    neighbourhood: Vec<(usize, [StackCoeff; 3])>,
//}

//impl From<SimplePatternConfig> for Pattern<ConcreteFiveLimitStackType> {
//    fn from(simple: SimplePatternConfig) -> Self {
//        let octave = Stack::from_pure_interval(ConcreteFiveLimitStackType::period_index(), 1);
//        Pattern {
//            name: simple.name,
//            keyshape: simple.keyshape,
//            neighbourhood: SomeNeighbourhood::PeriodicPartial(PeriodicPartial {
//                stacks: simple
//                    .neighbourhood
//                    .into_iter()
//                    .map(|(offset, coeffs)| (offset, Stack::from_target(coeffs.to_vec())))
//                    .collect(),
//                period: octave,
//            }),
//        }
//    }
//}

/// See the restrictions on [new_fivelimit_neighbourhood] on the first three arguments!
//pub fn init_walking_config(
//    initial_neighbourhood_width: StackCoeff,
//    initial_neighbourhood_index: StackCoeff,
//    initial_neighbourhood_offset: StackCoeff,
//    patterns: Vec<SimplePatternConfig>,
//) -> CompleteConfig<
//    ConcreteFiveLimitStackType,
//    Walking<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//    >,
//    WalkingConfig<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//    >,
//    Pitchbend12,
//    Pitchbend12Config,
//    WrappedGrid<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//        tui::grid::Grid<ConcreteFiveLimitStackType>,
//    >,
//    WrappedGridConfig<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//        tui::grid::Grid<ConcreteFiveLimitStackType>,
//        tui::grid::GridConfig<ConcreteFiveLimitStackType>,
//    >,
//> {
//    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
//    let initial_neighbourhood = new_fivelimit_neighbourhood(
//        &no_active_temperaments,
//        initial_neighbourhood_width,
//        initial_neighbourhood_index,
//        initial_neighbourhood_offset,
//    );
//    CompleteConfig {
//        midi_port_config: MidiPortConfig::AskAtStartup,
//        process_config: WalkingConfig {
//            _phantom: PhantomData,
//            temper_pattern_neighbourhoods: false,
//            use_patterns: true,
//            initial_neighbourhood: initial_neighbourhood.clone(),
//            patterns: patterns.into_iter().map(From::from).collect(),
//            consider_played: false,
//        },
//        backend_config: Pitchbend12Config {
//            channels: [
//                Channel::Ch1,
//                Channel::Ch2,
//                Channel::Ch3,
//                Channel::Ch4,
//                Channel::Ch5,
//                Channel::Ch6,
//                Channel::Ch7,
//                Channel::Ch8,
//                Channel::Ch9,
//                Channel::Ch11,
//                Channel::Ch12,
//                Channel::Ch13,
//            ],
//            bend_range: 2.0,
//        },
//        ui_config: WrappedGridConfig {
//            gridconfig: GridConfig {
//                display_config: DisplayConfig {
//                    notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
//                    color_range: 0.2,
//                    gradient: colorous::CIVIDIS,
//                },
//                initial_reference_key: 60,
//                initial_neighbourhood,
//                horizontal_index: 1,
//                vertical_index: 2,
//                fifth_index: 1,
//                third_index: 2,
//                _phantom: PhantomData,
//            },
//            latencyreporterconfig: LatencyReporterConfig { nsamples: 20 },
//            special_config: tui::grid::GridConfig {
//                notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
//                initial_key_center: Stack::new_zero(),
//                use_patterns: true,
//            },
//            _phantom: PhantomData,
//        },
//        _phantom: PhantomData,
//    }
//}

/// See the restrictions on [new_fivelimit_neighbourhood] on the first three arguments!
//pub fn init_walking_debug_config(
//    initial_neighbourhood_width: StackCoeff,
//    initial_neighbourhood_index: StackCoeff,
//    initial_neighbourhood_offset: StackCoeff,
//    patterns: Vec<SimplePatternConfig>,
//) -> CompleteConfig<
//    ConcreteFiveLimitStackType,
//    Walking<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//    >,
//    WalkingConfig<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//    >,
//    Pitchbend12,
//    Pitchbend12Config,
//    OnlyNotify,
//    OnlyNotifyConfig,
//> {
//    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
//    let initial_neighbourhood = new_fivelimit_neighbourhood(
//        &no_active_temperaments,
//        initial_neighbourhood_width,
//        initial_neighbourhood_index,
//        initial_neighbourhood_offset,
//    );
//    CompleteConfig {
//        midi_port_config: MidiPortConfig::AskAtStartup,
//        process_config: WalkingConfig {
//            _phantom: PhantomData,
//            temper_pattern_neighbourhoods: false,
//            use_patterns: true,
//            initial_neighbourhood: initial_neighbourhood.clone(),
//            patterns: patterns.into_iter().map(From::from).collect(),
//            consider_played: false,
//        },
//        backend_config: Pitchbend12Config {
//            channels: [
//                Channel::Ch1,
//                Channel::Ch2,
//                Channel::Ch3,
//                Channel::Ch4,
//                Channel::Ch5,
//                Channel::Ch6,
//                Channel::Ch7,
//                Channel::Ch8,
//                Channel::Ch9,
//                Channel::Ch11,
//                Channel::Ch12,
//                Channel::Ch13,
//            ],
//            bend_range: 2.0,
//        },
//        ui_config: OnlyNotifyConfig {},
//        _phantom: PhantomData,
//    }
//}

//pub fn init_fixed_spring_config(
//    initial_neighbourhood_width: StackCoeff,
//    initial_neighbourhood_index: StackCoeff,
//    initial_neighbourhood_offset: StackCoeff,
//    //patterns: Vec<SimplePatternConfig>,
//) -> CompleteConfig<
//    ConcreteFiveLimitStackType,
//    springs::fixed::State<ConcreteFiveLimitStackType, springs::fixed::ConcreteFiveLimitProvider>,
//    springs::fixed::Config,
//    Pitchbend12,
//    Pitchbend12Config,
//    WrappedGrid<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//        tui::grid::Grid<ConcreteFiveLimitStackType>,
//    >,
//    WrappedGridConfig<
//        ConcreteFiveLimitStackType,
//        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
//        tui::grid::Grid<ConcreteFiveLimitStackType>,
//        tui::grid::GridConfig<ConcreteFiveLimitStackType>,
//    >,
//> {
//    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
//    let initial_neighbourhood = new_fivelimit_neighbourhood(
//        &no_active_temperaments,
//        initial_neighbourhood_width,
//        initial_neighbourhood_index,
//        initial_neighbourhood_offset,
//    );
//    CompleteConfig {
//        midi_port_config: MidiPortConfig::AskAtStartup,
//        process_config: springs::fixed::Config {
//            initial_n_keys: 10,
//            initial_n_lengths: 90,
//            anchor_policy: springs::fixed::AnchorPolicy::AllConstants,
//            reference_window: Duration::from_millis(500),
//        },
//        backend_config: Pitchbend12Config {
//            channels: [
//                Channel::Ch1,
//                Channel::Ch2,
//                Channel::Ch3,
//                Channel::Ch4,
//                Channel::Ch5,
//                Channel::Ch6,
//                Channel::Ch7,
//                Channel::Ch8,
//                Channel::Ch9,
//                Channel::Ch11,
//                Channel::Ch12,
//                Channel::Ch13,
//            ],
//            bend_range: 2.0,
//        },
//        ui_config: WrappedGridConfig {
//            gridconfig: GridConfig {
//                display_config: DisplayConfig {
//                    notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
//                    color_range: 0.2,
//                    gradient: colorous::CIVIDIS,
//                },
//                initial_reference_key: 60,
//                initial_neighbourhood,
//                horizontal_index: 1,
//                vertical_index: 2,
//                fifth_index: 1,
//                third_index: 2,
//                _phantom: PhantomData,
//            },
//            latencyreporterconfig: LatencyReporterConfig { nsamples: 20 },
//            special_config: tui::grid::GridConfig {
//                notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
//                initial_key_center: Stack::new_zero(),
//                use_patterns: true,
//            },
//            _phantom: PhantomData,
//        },
//        _phantom: PhantomData,
//    }
//}

//pub fn init_fixed_spring_debug_config() -> CompleteConfig<
//    ConcreteFiveLimitStackType,
//    springs::fixed::State<ConcreteFiveLimitStackType, springs::fixed::ConcreteFiveLimitProvider>,
//    springs::fixed::Config,
//    Pitchbend12,
//    Pitchbend12Config,
//    OnlyNotify,
//    OnlyNotifyConfig,
//> {
//    CompleteConfig {
//        midi_port_config: MidiPortConfig::AskAtStartup,
//        process_config: springs::fixed::Config {
//            initial_n_keys: 10,
//            initial_n_lengths: 90,
//            anchor_policy: springs::fixed::AnchorPolicy::AllConstants,
//            reference_window: Duration::from_millis(500),
//        },
//        backend_config: Pitchbend12Config {
//            channels: [
//                Channel::Ch1,
//                Channel::Ch2,
//                Channel::Ch3,
//                Channel::Ch4,
//                Channel::Ch5,
//                Channel::Ch6,
//                Channel::Ch7,
//                Channel::Ch8,
//                Channel::Ch9,
//                Channel::Ch11,
//                Channel::Ch12,
//                Channel::Ch13,
//            ],
//            bend_range: 2.0,
//        },
//        ui_config: OnlyNotifyConfig {},
//        _phantom: PhantomData,
//    }
//}

pub fn init_static_config(
    initial_neighbourhood_width: StackCoeff,
    initial_neighbourhood_index: StackCoeff,
    initial_neighbourhood_offset: StackCoeff,
) -> CompleteConfig<
    ConcreteFiveLimitStackType,
    fromstrategy::State<
        ConcreteFiveLimitStackType,
        r#static::StaticTuning<
            ConcreteFiveLimitStackType,
            PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        >,
        r#static::StaticTuningConfig<ConcreteFiveLimitStackType>,
    >,
    fromstrategy::Config<
        ConcreteFiveLimitStackType,
        r#static::StaticTuning<
            ConcreteFiveLimitStackType,
            PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        >,
        r#static::StaticTuningConfig<ConcreteFiveLimitStackType>,
    >,
    Pitchbend12,
    Pitchbend12Config,
    WrappedGrid<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        latencyreporter::LatencyReporter,
        //tui::grid::Grid<
        //    ConcreteFiveLimitStackType,
        //    PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        //>,
    >,
    WrappedGridConfig<
        ConcreteFiveLimitStackType,
        neighbourhood::PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        latencyreporter::LatencyReporter,
        latencyreporter::LatencyReporterConfig,
        //tui::grid::Grid<
        //    ConcreteFiveLimitStackType,
        //    PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        //>,
        //tui::grid::GridConfig<
        //    ConcreteFiveLimitStackType,
        //    PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        //>,
    >,
> {
    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
    let initial_neighbourhood = new_fivelimit_neighbourhood(
        &no_active_temperaments,
        initial_neighbourhood_width,
        initial_neighbourhood_index,
        initial_neighbourhood_offset,
    );
    let global_reference = Reference::from_frequency(Stack::from_target(vec![1, -1, 1]), 440.0);
    CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: fromstrategy::Config {
            _phantom: PhantomData,
            strategy_config: r#static::StaticTuningConfig {
                active_temperaments: no_active_temperaments,
                width: initial_neighbourhood_width,
                index: initial_neighbourhood_index,
                offset: initial_neighbourhood_offset,
                global_reference,
            },
        },
        backend_config: Pitchbend12Config {
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
            ],
            bend_range: 2.0,
        },
        ui_config: WrappedGridConfig {
            gridconfig: GridConfig {
                display_config: DisplayConfig {
                    notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
                    color_range: 0.2,
                    gradient: colorous::CIVIDIS,
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
            special_config: LatencyReporterConfig { nsamples: 20 },
            //special_config: todo!(),
            _phantom: PhantomData,
        },
        _phantom: PhantomData,
    }
}

pub fn init_static_debug_config(
    initial_neighbourhood_width: StackCoeff,
    initial_neighbourhood_index: StackCoeff,
    initial_neighbourhood_offset: StackCoeff,
) -> CompleteConfig<
    ConcreteFiveLimitStackType,
    fromstrategy::State<
        ConcreteFiveLimitStackType,
        r#static::StaticTuning<
            ConcreteFiveLimitStackType,
            PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        >,
        r#static::StaticTuningConfig<ConcreteFiveLimitStackType>,
    >,
    fromstrategy::Config<
        ConcreteFiveLimitStackType,
        r#static::StaticTuning<
            ConcreteFiveLimitStackType,
            PeriodicCompleteAligned<ConcreteFiveLimitStackType>,
        >,
        r#static::StaticTuningConfig<ConcreteFiveLimitStackType>,
    >,
    Pitchbend12,
    Pitchbend12Config,
    OnlyNotify,
    OnlyNotifyConfig,
> {
    let no_active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
    let global_reference = Reference::from_frequency(Stack::from_target(vec![1, -1, 1]), 440.0);
    CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: fromstrategy::Config {
            _phantom: PhantomData,
            strategy_config: r#static::StaticTuningConfig {
                active_temperaments: no_active_temperaments,
                width: initial_neighbourhood_width,
                index: initial_neighbourhood_index,
                offset: initial_neighbourhood_offset,
                global_reference,
            },
        },
        backend_config: Pitchbend12Config {
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
            ],
            bend_range: 2.0,
        },
        ui_config: OnlyNotifyConfig {},
        _phantom: PhantomData,
    }
}
