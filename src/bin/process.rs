use serde_json;
use std::fs;

use adaptuner::{
    config::{validate, Config, RawConfig},
    interval::StackType,
    pattern::{KeyShape, Pattern},
    process,
    util::{dimension::fixed_sizes::Size3, mod12::PitchClass::*},
};

#[derive(Debug, Copy, Clone)]
struct TTag {}

pub fn main() {
    let raw_config: RawConfig =
        serde_json::from_str(&fs::read_to_string("config.json").unwrap()).unwrap();
    let config: Config<Size3, TTag> = validate(raw_config);
    let stype = StackType::new(config.intervals.clone(), config.temperaments.clone());

    println!("{config:?}");

    // let st = process::State {
    //     birthday: 0,
    //
    //     active_notes: [false; 128],
    //     sustain: false,
    //
    //     config: process::Config {
    //         patterns: &config.patterns,
    //         minimum_age: 10000,
    //     },
    // };
}
