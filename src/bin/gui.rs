use std::error::Error;

use midi_msg::Channel;

use adaptuner::{
    backend::pitchbend12::{Pitchbend12, Pitchbend12Config},
    config::{Config, StrategyConfig},
    gui::{
        latticewindow::LatticeWindowConfig, manywindows::ManyWindows,
        referencewindow::ReferenceWindowConfig, tuningreferencewindow::TuningReferenceWindowConfig,
    },
    interval::stacktype::fivelimit::{TheFiveLimitStackType, DIESIS_SYNTONIC},
    notename::NoteNameStyle,
    process::fromstrategy::ProcessFromStrategy,
    run::RunState,
    strategy::r#static::*,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut config: Config<TheFiveLimitStackType> =
        serde_yml::from_reader(std::fs::File::open("conf.yaml")?)?;
    TheFiveLimitStackType::initialise(&config.temperaments)?;

    let backend_config = Pitchbend12Config {
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
            // Channel::Ch10,
            Channel::Ch11,
            Channel::Ch12,
            Channel::Ch13,
            // Channel::Ch14,
            // Channel::Ch15,
            // Channel::Ch16,
        ],
        bend_range: 2.0,
    };

    let backend_window_config = backend_config.clone();

    let lattice_window_config = LatticeWindowConfig {
        zoom: 10.0,
        flatten: 1.0,
        interval_heights: vec![
            0.0,
            -12.0 * (5.0 / 4.0 as f32).log2(),
            12.0 * (3.0 / 2.0 as f32).log2(),
        ],
        background_stack_distances: vec![0, 3, 2],
    };

    let notenamestyle = NoteNameStyle::JohnstonFiveLimitFull;
    let correction_system_index = DIESIS_SYNTONIC;
    let reference_window_config = ReferenceWindowConfig {
        notenamestyle,
        correction_system_index,
    };
    let tuning_reference_window_config = TuningReferenceWindowConfig {
        notenamestyle,
        correction_system_index,
    };

    let latency_window_length = 20;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let first_config = config.strategies.drain(..).next().unwrap();
    //     StrategyConfig::StaticTuning(sc) => sc,
    // };
    let static_tuning_config = match first_config {
        StrategyConfig::StaticTuning(x) => x,
    };
    let static_tuning = StaticTuning::new(static_tuning_config);

    let _runstate = RunState::new(
        midi_in,
        midi_out,
        || ProcessFromStrategy::new(static_tuning),
        move || Pitchbend12::new(backend_config),
        move |ctx, tx| {
            ManyWindows::new(
                lattice_window_config,
                reference_window_config,
                backend_window_config,
                latency_window_length,
                tuning_reference_window_config,
                ctx,
                tx,
            )
        },
    );

    Ok(())
}
