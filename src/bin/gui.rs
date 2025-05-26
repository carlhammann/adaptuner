use std::error::Error;

use midi_msg::Channel;

use adaptuner::{
    backend::{pitchbend::{Pitchbend, PitchbendConfig}, pitchbend12::{Pitchbend12, Pitchbend12Config}},
    gui::manywindows::ManyWindows,
    interval::{stack::Stack, stacktype::fivelimit::ConcreteFiveLimitStackType},
    notename::NoteNameStyle,
    process::fromstrategy::ProcessFromStrategy,
    reference::Reference,
    run::RunState,
    strategy::r#static::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let global_reference = Reference::<ConcreteFiveLimitStackType>::from_frequency(
        Stack::from_target(vec![1, -1, 1]),
        440.0,
    );
    let notenamestyle = NoteNameStyle::JohnstonFiveLimitFull;
    let no_active_temperaments = vec![false; 2];
    let initial_neighbourhood_width = 4;
    let initial_neighbourhood_index = 5;
    let initial_neighbourhood_offset = 1;
    let strategy_config = StaticTuningConfig {
        active_temperaments: no_active_temperaments,
        width: initial_neighbourhood_width,
        index: initial_neighbourhood_index,
        offset: initial_neighbourhood_offset,
        global_reference: global_reference.clone(),
    };

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
            Channel::Ch11,
            Channel::Ch12,
            Channel::Ch13,
        ],
        bend_range: 2.0,
    };

    let latency_window_length = 20;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let _runstate = RunState::new(
        midi_in,
        midi_out,
        || ProcessFromStrategy::new(StaticTuning::new(strategy_config)),
        move || Pitchbend::new(&backend_config),
        move |ctx, tx| {
            ManyWindows::new(
                ctx,
                latency_window_length,
                global_reference,
                notenamestyle,
                tx,
            )
        },
    );

    Ok(())
}
