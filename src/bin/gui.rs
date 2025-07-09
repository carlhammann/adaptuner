use std::error::Error;

use midi_msg::Channel;

use adaptuner::{
    backend::pitchbend12::{Pitchbend12, Pitchbend12Config},
    gui::{latticewindow::LatticeWindowConfig, manywindows::ManyWindows},
    interval::{stack::Stack, stacktype::fivelimit::ConcreteFiveLimitStackType},
    neighbourhood::{Neighbourhood, PeriodicCompleteAligned},
    notename::NoteNameStyle,
    process::fromstrategy::ProcessFromStrategy,
    reference::Reference,
    run::RunState,
    strategy::r#static::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let tuning_reference = Reference::<ConcreteFiveLimitStackType>::from_frequency(
        Stack::from_target(vec![1, -1, 1]),
        440.0,
    );
    let notenamestyle = NoteNameStyle::JohnstonFiveLimitFull;
    let interval_heights = vec![
        0.0,
        -12.0 * (5.0 / 4.0 as f32).log2(),
        12.0 * (3.0 / 2.0 as f32).log2(),
    ];
    let background_stack_distances = vec![0, 3, 2];
    let no_active_temperaments = vec![false; 2];
    let initial_neighbourhoods = vec![PeriodicCompleteAligned::from_octave_tunings(
        [
            Stack::new_zero(),                  // C
            Stack::from_target(vec![0, -1, 2]), // C#
            Stack::from_target(vec![-1, 2, 0]), // D
            Stack::from_target(vec![0, 1, -1]), // Eb
            Stack::from_target(vec![0, 0, 1]),  // E
            Stack::from_target(vec![1, -1, 0]), // F
            Stack::from_target(vec![-1, 2, 1]), // F#
            Stack::from_target(vec![0, 1, 0]),  // G
            Stack::from_target(vec![0, 0, 2]),  // G#
            Stack::from_target(vec![1, -1, 1]), // A
            Stack::from_target(vec![0, 2, -1]), // Bb
            Stack::from_target(vec![0, 1, 1]),  // B
        ],
        "Reihe 1".into(),
    )];
    let initial_reference = Stack::new_zero();

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

    let latticewindow_config = LatticeWindowConfig {
        tuning_reference: tuning_reference.clone(),
        active_temperaments: no_active_temperaments,
        reference: initial_reference.clone(),
        initial_considered_notes: initial_neighbourhoods[0].clone(),
        initial_neighbourhood_name: initial_neighbourhoods[0].name().into(),
        initial_neighbourhood_index: 0,
        zoom: 10.0,
        flatten: 1.0,
        notenamestyle,
        interval_heights,
        background_stack_distances,
    };

    let latency_window_length = 20;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let static_tuning = StaticTuning::new(
        tuning_reference.clone(),
        initial_reference.clone(),
        initial_neighbourhoods
    );

    let _runstate = RunState::new(
        midi_in,
        midi_out,
        || ProcessFromStrategy::new(static_tuning),
        move || Pitchbend12::new(&backend_config),
        move |ctx, tx| {
            ManyWindows::new(
                latticewindow_config,
                latency_window_length,
                tuning_reference,
                initial_reference,
                notenamestyle,
                ctx,
                tx,
            )
        },
    );

    Ok(())
}
