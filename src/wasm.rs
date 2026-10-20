#![cfg(target_arch = "wasm32")]

use std::{
    panic,
    sync::{mpsc, Arc},
};

use crate::time::{Duration, Instant};

use midi_msg::Channel;
use parking_lot::RwLock;
use serde::Deserialize;

use web_sys::console;

use wasm_bindgen::prelude::*;

use crate::{
    adaptors::ConcreteLocks,
    backend::pitchbend12::{Pitchbend12, Pitchbend12Config},
    backend::r#trait::BackendAdaptorNew,
    config::{BackendConfig, Config, GuiConfig, StrategyConfig},
    interval::{
        base::Semitones,
        fundamental::{HasFundamental, HasOvertone},
        stack::Stack,
        stacktype::fivelimit::TheFiveLimitStackType,
        stacktype::r#trait::{
            FiveLimitIntervalBasis, OctavePeriodicStackType, Reloadable, StackType,
        },
    },
    keystate::KeyState,
    msg::{FromBackend, FromProcess, MessageTranslate2, ReceiveMsg, ToBackend, ToProcess},
    process::{
        fromstrategy::ProcessFromStrategy,
        r#trait::{ProcessAdaptor, StackWithTuning},
    },
    reference::Reference,
    strategy::{
        harmony::r#trait::Harmony,
        melody::r#trait::{Anchoring, AnchoringKind},
    },
    util::ordered_locks::{OrderedLocks, Zero},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmMidiEvent {
    pub time: u64,
    pub channel: u8,
    pub message: WasmMidiMessage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WasmMidiMessage {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8, velocity: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmMidiOutput {
    pub time: u64,
    pub bytes: Vec<u8>,
}

pub struct WasmRuntime<T>
where
    T: StackType + Send + Sync + FiveLimitIntervalBasis + HasFundamental + HasOvertone,
{
    process: ProcessFromStrategy<T>,
    backend: Pitchbend12<T>,

    from_process_rx: mpsc::Receiver<FromProcess<T>>,
    from_backend_rx: mpsc::Receiver<FromBackend>,

    output: Vec<WasmMidiOutput>,

    current_time: Instant,
}

impl<T> WasmRuntime<T>
where
    T: StackType
        + Send
        + Sync
        + FiveLimitIntervalBasis
        + HasFundamental
        + HasOvertone
        + Reloadable
        + OctavePeriodicStackType,
{
    pub fn new(
        tuning_reference: Reference<T>,
        strategies: Vec<StrategyConfig<T>>,
        backend_config: Pitchbend12Config,
        gui_config: GuiConfig,
    ) -> Self {
        let (from_process_tx, from_process_rx) = mpsc::channel::<FromProcess<T>>();
        let (from_backend_tx, from_backend_rx) = mpsc::channel::<FromBackend>();

        // The UI channel is currently unused in WASM, but ConcreteLocks
        // requires its sender to exist.
        let (from_ui_tx, _from_ui_rx) = mpsc::channel();

        let now = Instant::now();
        crate::time::set_now(now);

        let locks = Arc::new(ConcreteLocks {
            from_process_tx,
            from_ui_tx,
            from_backend_tx,

            pedal_hold: RwLock::new([false; 16]),
            sostenuto_hold: RwLock::new([false; 16]),
            soft_hold: RwLock::new([false; 16]),

            tunings: core::array::from_fn(|i| {
                RwLock::new(StackWithTuning {
                    stack: Stack::new_zero(),
                    semitones: i as Semitones,
                })
            }),

            key_states: core::array::from_fn(|_| RwLock::new(KeyState::new(now))),

            reference: RwLock::new(Stack::new_zero()),
            tuning_reference: RwLock::new(tuning_reference),

            strategy_config: RwLock::new(strategies),
            active_strategy_index: RwLock::new(0),

            harmony: RwLock::new(Harmony::None),

            anchoring: RwLock::new(Anchoring {
                kind: AnchoringKind::None,
                key: 60,
                stack: Stack::new_zero(),
            }),

            backend_config: RwLock::new(backend_config),
            gui_config: RwLock::new(gui_config),
        });

        let process_adaptor: ProcessAdaptor<T, Zero> =
            unsafe { OrderedLocks::new_zero(locks.clone()) };

        let backend_adaptor: BackendAdaptorNew<T, Zero> = unsafe { OrderedLocks::new_zero(locks) };

        let mut process = ProcessFromStrategy::new(process_adaptor);
        let backend = Pitchbend12::new(backend_adaptor);

        process.receive_msg(ToProcess::Start { time: now });

        Self {
            process,
            backend,
            from_process_rx,
            from_backend_rx,
            output: Vec::new(),
            current_time: now,
        }
    }

    fn parse_channel(channel: u8) -> Result<Channel, String> {
        match channel {
            0 => Ok(Channel::Ch1),
            1 => Ok(Channel::Ch2),
            2 => Ok(Channel::Ch3),
            3 => Ok(Channel::Ch4),
            4 => Ok(Channel::Ch5),
            5 => Ok(Channel::Ch6),
            6 => Ok(Channel::Ch7),
            7 => Ok(Channel::Ch8),
            8 => Ok(Channel::Ch9),
            9 => Ok(Channel::Ch10),
            10 => Ok(Channel::Ch11),
            11 => Ok(Channel::Ch12),
            12 => Ok(Channel::Ch13),
            13 => Ok(Channel::Ch14),
            14 => Ok(Channel::Ch15),
            15 => Ok(Channel::Ch16),
            _ => Err(format!(
                "MIDI channel must be between 0 and 15, got {channel}"
            )),
        }
    }

    fn channel_to_u8(channel: Channel) -> u8 {
        match channel {
            Channel::Ch1 => 0,
            Channel::Ch2 => 1,
            Channel::Ch3 => 2,
            Channel::Ch4 => 3,
            Channel::Ch5 => 4,
            Channel::Ch6 => 5,
            Channel::Ch7 => 6,
            Channel::Ch8 => 7,
            Channel::Ch9 => 8,
            Channel::Ch10 => 9,
            Channel::Ch11 => 10,
            Channel::Ch12 => 11,
            Channel::Ch13 => 12,
            Channel::Ch14 => 13,
            Channel::Ch15 => 14,
            Channel::Ch16 => 15,
        }
    }

    pub fn process_events(
        &mut self,
        events: &[WasmMidiEvent],
    ) -> Result<Vec<WasmMidiOutput>, String> {
        console::log_1(&format!("WASM: process_events START: {} events", events.len()).into());

        self.output.clear();

        let total_start = Instant::now();

        let base_time = self.current_time;
        let mut previous_time = 0u64;

        let mut process_event_total = Duration::ZERO;
        let mut drain_process_total = Duration::ZERO;
        let mut drain_backend_total = Duration::ZERO;

        for (index, event) in events.iter().enumerate() {
            if event.time < previous_time {
                return Err(format!(
                    "MIDI events must be sorted by time: {} came after {}",
                    event.time, previous_time
                ));
            }

            previous_time = event.time;

            let time = base_time + Duration::from_micros(event.time);

            crate::time::set_now(time);

            let event_start = Instant::now();

            let start = Instant::now();
            self.process_event(event, time)?;
            let process_event_duration = start.elapsed();

            let start = Instant::now();
            self.drain_process_messages(base_time)?;
            let drain_process_duration = start.elapsed();

            let start = Instant::now();
            self.drain_backend_messages(base_time);
            let drain_backend_duration = start.elapsed();

            self.current_time = time;

            let total_duration = event_start.elapsed();

            if total_duration > Duration::from_millis(10) {
                console::log_1(
            &format!(
                "WASM SLOW EVENT: index={} time={} message={:?} total={:?} process={:?} drain_process={:?} drain_backend={:?}",
                index,
                event.time,
                event.message,
                total_duration,
                process_event_duration,
                drain_process_duration,
                drain_backend_duration,
            )
            .into(),
        );
            }

            if (index + 1) % 25 == 0 {
                console::log_1(
                    &format!("WASM PROGRESS: {}/{} events", index + 1, events.len()).into(),
                );
            }
        }

        let total_duration = total_start.elapsed();

        console::log_1(
            &format!(
                "WASM: process_events END: {} events -> {} outputs",
                events.len(),
                self.output.len()
            )
            .into(),
        );

        Ok(std::mem::take(&mut self.output))
    }

    fn process_event(&mut self, event: &WasmMidiEvent, time: Instant) -> Result<(), String> {
        let channel = Self::parse_channel(event.channel)?;

        match &event.message {
            WasmMidiMessage::NoteOn { note, velocity } => {
                let start = Instant::now();

                self.process.receive_msg(ToProcess::NoteOn {
                    channel,
                    note: *note,
                    velocity: *velocity,
                    time,
                });

                let elapsed = start.elapsed();

                if elapsed > Duration::from_millis(5) {
                    console::log_1(
                        &format!(
                            "WASM SLOW receive_msg: note={} velocity={} duration={:?}",
                            note, velocity, elapsed
                        )
                        .into(),
                    );
                }
            }

            WasmMidiMessage::NoteOff { note, velocity } => {
                self.process.receive_msg(ToProcess::NoteOff {
                    channel,
                    note: *note,
                    velocity: *velocity,
                    time,
                });
            }

            WasmMidiMessage::ControlChange { controller, value } => {
                self.process.receive_msg(ToProcess::IncomingMidi {
                    time,
                    bytes: vec![0xB0 | Self::channel_to_u8(channel), *controller, *value],
                });
            }

            WasmMidiMessage::ProgramChange { program } => {
                self.process.receive_msg(ToProcess::IncomingMidi {
                    time,
                    bytes: vec![0xC0 | Self::channel_to_u8(channel), *program],
                });
            }
        }

        Ok(())
    }

    fn drain_process_messages(&mut self, base_time: Instant) -> Result<(), String> {
        while let Ok(msg) = self.from_process_rx.try_recv() {
            match msg {
                FromProcess::OutgoingMidi { bytes, time } => {
                    self.output.push(WasmMidiOutput {
                        time: time.duration_since(base_time).as_micros() as u64,
                        bytes,
                    });
                }

                FromProcess::NoteOn {
                    channel,
                    note,
                    velocity,
                    time,
                } => {
                    self.backend.receive_msg(ToBackend::NoteOn {
                        channel,
                        note,
                        velocity,
                        time,
                    });
                }

                FromProcess::NoteOff {
                    channel,
                    note,
                    velocity,
                    time,
                } => {
                    self.backend.receive_msg(ToBackend::NoteOff {
                        channel,
                        note,
                        velocity,
                        time,
                    });
                }

                FromProcess::PedalHold {
                    channel,
                    value,
                    time,
                } => {
                    self.backend.receive_msg(ToBackend::PedalHold {
                        channel,
                        value,
                        time,
                    });
                }

                FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                } => {
                    self.backend.receive_msg(ToBackend::ProgramChange {
                        channel,
                        program,
                        time,
                    });
                }

                FromProcess::FromStrategy(msg) => {
                    let (backend_msg, _) = msg.translate2();

                    if let Some(backend_msg) = backend_msg {
                        self.backend.receive_msg(backend_msg);
                    }
                }

                FromProcess::MidiParseErr(error) => {
                    return Err(error);
                }

                FromProcess::StartedStrategy { time, .. } => {
                    self.backend.receive_msg(ToBackend::Reset { time });
                }
            }
        }

        Ok(())
    }

    fn drain_backend_messages(&mut self, base_time: Instant) {
        while let Ok(msg) = self.from_backend_rx.try_recv() {
            match msg {
                FromBackend::OutgoingMidi { time, bytes } => {
                    self.output.push(WasmMidiOutput {
                        time: time.duration_since(base_time).as_micros() as u64,
                        bytes,
                    });
                }

                FromBackend::DetunedNote { .. } => {
                    // UI information in the native runtime.
                    // Not needed for the first WASM MIDI API.
                }
            }
        }
    }
}

const TEMPLATE_CONFIG: &str = include_str!("../configs/chords_and_springs.yaml");

#[wasm_bindgen]
pub struct AdaptunerEngine {
    runtime: WasmRuntime<TheFiveLimitStackType>,
}

#[wasm_bindgen]
impl AdaptunerEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<AdaptunerEngine, JsValue> {
        panic::set_hook(Box::new(|info| {
            console::error_1(&format!("Adaptuner WASM panic: {info}").into());
        }));

        println!("WASM: constructor start");

        let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        println!("WASM: config parsed");

        TheFiveLimitStackType::initialise(config.temperaments, config.named_intervals)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        println!("WASM: stack initialized");

        let p12_config = match config.backend {
            BackendConfig::Pitchbend12(config) => config,
        };

        println!("WASM: backend config extracted");

        let runtime = WasmRuntime::new(
            config.tuning_reference,
            config.strategies,
            p12_config,
            config.gui,
        );

        println!("WASM: runtime created");

        Ok(Self { runtime })
    }

    pub fn process(&mut self, events: JsValue) -> Result<JsValue, JsValue> {
        let events: Vec<WasmMidiEvent> = serde_wasm_bindgen::from_value(events)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        console::log_1(&format!("WASM: BEFORE process_events: {} events", events.len()).into());

        let output = self
            .runtime
            .process_events(&events)
            .map_err(|error| JsValue::from_str(&error))?;

        console::log_1(&format!("WASM: AFTER process_events: {} outputs", output.len()).into());

        serde_wasm_bindgen::to_value(&output).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    use crate::{
        config::{BackendConfig, Config},
        interval::stacktype::{fivelimit::TheFiveLimitStackType, r#trait::Reloadable},
    };

    #[wasm_bindgen_test]
    fn processes_note_on_through_full_pipeline() {
        let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG).unwrap();

        TheFiveLimitStackType::initialise(config.temperaments, config.named_intervals).unwrap();

        let p12_config = match config.backend {
            BackendConfig::Pitchbend12(c) => c,
        };

        let mut runtime = WasmRuntime::new(
            config.tuning_reference,
            config.strategies,
            p12_config,
            config.gui,
        );

        let output = runtime
            .process_events(&[WasmMidiEvent {
                time: 0,
                channel: 0,
                message: WasmMidiMessage::NoteOn {
                    note: 60,
                    velocity: 100,
                },
            }])
            .unwrap();

        println!("WASM output: {output:?}");

        assert!(!output.is_empty(), "expected MIDI output for NoteOn");
    }
}
