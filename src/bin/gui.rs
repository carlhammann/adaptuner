use std::error::Error;

use eframe::egui;
use midi_msg::Channel;

use adaptuner::{
    backend::pitchbend::{Pitchbend, PitchbendConfig},
    gui::manywindows::ManyWindows,
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
    let interval_colours = vec![
        egui::Color32::RED,
        egui::Color32::GREEN,
        egui::Color32::BLUE,
    ];
    let background_stack_distances = vec![0, 2, 2];
    let no_active_temperaments = vec![false; 2];
    let initial_neighbourhood = PeriodicCompleteAligned::from_octave_tunings([
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
    ]);
    let initial_reference = Stack::new_zero();

    let backend_config = PitchbendConfig {
        channels: vec![
            Channel::Ch1,
            Channel::Ch2,
            Channel::Ch3,
            Channel::Ch4,
            Channel::Ch5,
            Channel::Ch6,
            Channel::Ch7,
            Channel::Ch8,
            Channel::Ch9,
            Channel::Ch10,
            Channel::Ch11,
            Channel::Ch12,
            Channel::Ch13,
            Channel::Ch14,
            Channel::Ch15,
            Channel::Ch16,
        ],
        bend_range: 2.0,
    };

    let latency_window_length = 20;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let static_tuning = StaticTuning::new(
        tuning_reference.clone(),
        initial_reference.clone(),
        no_active_temperaments.clone(),
        initial_neighbourhood.clone(),
    );

    let _runstate = RunState::new(
        midi_in,
        midi_out,
        || ProcessFromStrategy::new(static_tuning),
        move || Pitchbend::new(&backend_config),
        move |ctx, tx| {
            ManyWindows::new(
                ctx,
                latency_window_length,
                tuning_reference,
                no_active_temperaments,
                initial_reference,
                initial_neighbourhood,
                notenamestyle,
                interval_heights,
                interval_colours,
                background_stack_distances,
                tx,
            )
        },
    );

    Ok(())
}
