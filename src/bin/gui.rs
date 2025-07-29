use std::error::Error;

use adaptuner::{
    backend::pitchbend12::Pitchbend12,
    config::Config,
    gui::toplevel::Toplevel,
    interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::Reloadable},
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
        serde_yml::from_reader(std::fs::File::open("minimal.yaml")?)?;
    TheFiveLimitStackType::initialise(&config.temperaments, &config.named_intervals)?;

    let (process_config, gui_config, backend_config) = config.split();

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let runstate = RunState::new::<ProcessFromStrategy<TheFiveLimitStackType>, Pitchbend12, _, _>(
        midi_in,
        midi_out,
        process_config,
        backend_config,
        move |ctx, tx| Toplevel::new(gui_config, ctx, tx),
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
        ))
        .unwrap()
    );

    Ok(())
}
