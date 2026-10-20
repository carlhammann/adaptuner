#[cfg(not(target_arch = "wasm32"))]
use std::error::Error;

#[cfg(not(target_arch = "wasm32"))]
use adaptuner::{
    config::{BackendConfig, Config},
    interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::Reloadable},
    run::RunState,
};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
const TEMPLATE_CONFIG: &'static str = include_str!("../configs/template.yaml");

#[cfg(not(target_arch = "wasm32"))]
fn run() -> Result<(), Box<dyn Error>> {
    let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG)?;
    TheFiveLimitStackType::initialise(config.temperaments, config.named_intervals)?;

    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;

    let p12_config = match config.backend {
        BackendConfig::Pitchbend12(c) => c,
    };

    let _runstate = RunState::new(
        midi_in,
        midi_out,
        config.tuning_reference,
        config.strategies,
        p12_config,
        config.gui,
    )?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
