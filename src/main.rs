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

const TEMPLATE_CONFIG: &'static str = include_str!("../configs/template.yaml");

fn run() -> Result<(), Box<dyn Error>> {
    let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG)?;
    let (process_config, gui_config, backend_config) = config.split();
    TheFiveLimitStackType::initialise(config.temperaments, config.named_intervals)?;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let _runstate = RunState::new::<ProcessFromStrategy<TheFiveLimitStackType>, Pitchbend12, _, _>(
        midi_in,
        midi_out,
        process_config,
        backend_config,
        move |ctx, tx| Toplevel::new(gui_config, ctx, tx),
    )?;

    Ok(())
}
