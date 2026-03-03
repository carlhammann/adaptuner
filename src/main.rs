use std::error::Error;

use adaptuner::{
    backend::pitchbend12::Pitchbend12,
    config::Config,
    interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::Reloadable},
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
    TheFiveLimitStackType::initialise(config.temperaments, config.named_intervals)?;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let _runstate = RunState::new::<Pitchbend12<_>>(
        midi_in,
        midi_out,
        config.strategies,
        config.backend,
    )?;

    Ok(())
}
