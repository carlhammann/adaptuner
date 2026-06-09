use std::error::Error;

use adaptuner::{
    backend::pitchbend12::Pitchbend12,
    config::{BackendConfig, Config},
    interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::Reloadable},
    run::RunState,
};

fn main() {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        for deadlock in parking_lot::deadlock::check_deadlock() {
            for deadlock in deadlock {
                println!(
                    "Found a deadlock! {:#?}:\n{:?}",
                    deadlock.thread_id(),
                    deadlock.backtrace()
                );
            }
        }
    });

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

    let p12_config = match config.backend {
        BackendConfig::Pitchbend12(c) => c,
    };

    let _runstate = RunState::new(midi_in, midi_out, config.strategies, p12_config, config.gui)?;

    Ok(())
}
