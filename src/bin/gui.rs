use std::error::Error;

use midi_msg::Channel;

use adaptuner::{
    backend::pitchbend12::Pitchbend12,
    config::{Config, GuiConfig},
    gui::{
        common::CorrectionSystemChooser,
        editor::{reference::ReferenceEditorConfig, tuning::TuningEditorConfig},
        lattice::LatticeWindowControls,
        toplevel::Toplevel,
    },
    interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::StackType},
    notename::NoteNameStyle,
    process::fromstrategy::ProcessFromStrategy,
    run::RunState,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config: Config<TheFiveLimitStackType> =
        serde_yml::from_reader(std::fs::File::open("conf.yaml")?)?;
    TheFiveLimitStackType::initialise(&config.temperaments, &config.named_intervals)?;

    let (
        process_config,
        GuiConfig {
            strategies: strategy_names_and_bindings,
        },
        backend_config,
    ) = config.split();

    let backend_window_config = backend_config.clone();

    let lattice_window_config = LatticeWindowControls {
        zoom: 10.0,
        notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
        interval_heights: vec![
            0.0,
            12.0 * (5.0 / 4.0 as f32).log2(),
            -12.0 * (3.0 / 2.0 as f32).log2(),
        ],
        background_stack_distances: vec![0, 3, 2],
        project_dimension: 0,
        screen_keyboard_center: 60,
        screen_keyboard_channel: Channel::Ch1,
        screen_keyboard_velocity: 64,
        screen_keyboard_pedal_hold: false,
        highlight_playable_keys: false,
        correction_system_chooser: CorrectionSystemChooser::new(
            "lattice window correction system chooser",
        ),
    };
    let reference_window_config = ReferenceEditorConfig {
        notenamestyle: NoteNameStyle::JohnstonFiveLimitFull,
    };
    let tuning_reference_window_config = TuningEditorConfig {
        notenamestyle: NoteNameStyle::JohnstonFiveLimitFull,
    };

    let latency_window_length = 20;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let runstate = RunState::new::<ProcessFromStrategy<TheFiveLimitStackType>, Pitchbend12, _, _>(
        midi_in,
        midi_out,
        process_config,
        backend_config,
        move |ctx, tx| {
            Toplevel::new(
                strategy_names_and_bindings,
                lattice_window_config,
                reference_window_config,
                backend_window_config,
                latency_window_length,
                tuning_reference_window_config,
                ctx,
                tx,
            )
        },
    )?;

    let (process_config, backend_config, gui_config, _, _) = runstate.stop()?;

    // println!("{}", serde_yml::to_string(&process_config).unwrap());
    // println!("\n\n\n\n");
    // println!("{}", serde_yml::to_string(&backend_config).unwrap());
    //
    println!(
        "{}",
        serde_yml::to_string(&Config::join(
            process_config,
            backend_config,
            gui_config,
            config.temperaments,
            config.named_intervals
        )).unwrap()
    );

    Ok(())
}
