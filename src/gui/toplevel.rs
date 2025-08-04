use std::{cell::RefCell, hash::Hash, rc::Rc, sync::mpsc, time::Instant};

use eframe::{self, egui};
use serde::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, GuiConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, Reloadable, StackType},
    },
    keystate::KeyState,
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
    notename::HasNoteNames,
    reference::Reference,
};

use super::{
    backend::BackendWindow,
    common::{show_hide_button, CorrectionSystemChooser, SmallFloatingWindow},
    config::ConfigFileDialog,
    connection::{ConnectionWindow, Input, Output},
    latency::LatencyWindow,
    lattice::LatticeWindow,
    latticecontrol::AsKeyboardControls,
    notes::NoteWindow,
    r#trait::GuiShow,
    strategy::{AsStrategyPicker, AsWindows, StrategyWindows},
};

pub struct KeysAndTunings<T: IntervalBasis> {
    pub active_notes: [KeyState; 128],
    pub pedal_hold: [bool; 16],
    pub tunings: [Stack<T>; 128],
    pub reference: Stack<T>,
    pub tuning_reference: Reference<T>,
}

impl<T: IntervalBasis> KeysAndTunings<T> {
    fn new(time: Instant) -> Self {
        Self {
            active_notes: core::array::from_fn(|_| KeyState::new(time)),
            pedal_hold: [false; 16],
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            reference: Stack::new_zero(),
            tuning_reference: Reference {
                stack: Stack::new_zero(),
                semitones: 60.0,
            },
        }
    }
}

pub struct Toplevel<T: StackType> {
    state: KeysAndTunings<T>,

    lattice: LatticeWindow<T>,
    show_keyboard_controls: bool,

    strategies: StrategyWindows<T>,

    input_connection: ConnectionWindow<Input>,
    output_connection: ConnectionWindow<Output>,
    connection_window: SmallFloatingWindow,

    backend: BackendWindow,

    latency: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,

    notes: NoteWindow<T>,
    note_window: SmallFloatingWindow,

    config_file_dialog: ConfigFileDialog<T>,
}

impl<T: StackType + HasNoteNames + Hash + Serialize> Toplevel<T> {
    pub fn new(config: GuiConfig<T>, ctx: &egui::Context, tx: mpsc::Sender<FromUi<T>>) -> Self {
        let correction_system_chooser = Rc::new(RefCell::new(CorrectionSystemChooser::new(
            "correction_system_chooser",
            config.use_cent_values,
        )));

        Self {
            state: KeysAndTunings::new(Instant::now()),
            strategies: StrategyWindows::new(
                config.strategies,
                config.tuning_editor,
                config.reference_editor,
                correction_system_chooser.clone(),
            ),

            lattice: LatticeWindow::new(config.lattice_window, correction_system_chooser),
            show_keyboard_controls: false,

            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            connection_window: SmallFloatingWindow::new(egui::Id::new("connection_window")),
            backend: BackendWindow::new(config.backend_window),
            latency: LatencyWindow::new(config.latency_mean_over),
            notes: NoteWindow::new(ctx),
            note_window: SmallFloatingWindow::new(egui::Id::new("note_window")),
            tx,
            config_file_dialog: ConfigFileDialog::new(),
        }
    }

    fn restart_from_config(&mut self, config: GuiConfig<T>, time: Instant) {
        let correction_system_chooser = Rc::new(RefCell::new(CorrectionSystemChooser::new(
            "correction_system_chooser",
            config.use_cent_values,
        )));

        self.state = KeysAndTunings::new(time);

        self.lattice
            .restart_from_config(config.lattice_window, correction_system_chooser.clone());

        self.strategies.restart_from_config(
            config.strategies,
            config.tuning_editor,
            config.reference_editor,
            correction_system_chooser,
            time,
        );
        // input, output, latency don't need a restart

        self.backend
            .restart_from_config(config.backend_window, time);

        // self.notes.restart_from_config(config.notes_window, time);
    }
}

impl<T: StackType + Serialize> HandleMsg<ToUi<T>, FromUi<T>> for Toplevel<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        match &msg {
            ToUi::NoteOn {
                time,
                channel,
                note,
            } => {
                self.state.active_notes[*note as usize].note_on(*channel, *time);
            }

            ToUi::NoteOff {
                time,
                channel,
                note,
            } => {
                self.state.active_notes[*note as usize].note_off(
                    *channel,
                    self.state.pedal_hold[*channel as usize],
                    *time,
                );
            }

            ToUi::PedalHold {
                channel,
                value,
                time,
            } => {
                self.state.pedal_hold[*channel as usize] = *value != 0;
                if *value == 0 {
                    for n in self.state.active_notes.iter_mut() {
                        n.pedal_off(*channel, *time);
                    }
                }
            }

            ToUi::Retune { note, tuning_stack } => {
                self.state.tunings[*note as usize].clone_from(tuning_stack);
            }

            ToUi::TunedNoteOn {
                time,
                channel,
                note,
                tuning_stack,
            } => {
                self.state.active_notes[*note as usize].note_on(*channel, *time);
                self.state.tunings[*note as usize].clone_from(tuning_stack);
            }
            _ => {}
        }

        self.lattice.handle_msg_ref(&msg, forward);
        self.notes.handle_msg_ref(&msg, forward);
        self.strategies.handle_msg_ref(&msg, forward);
        self.input_connection.handle_msg_ref(&msg, forward);
        self.output_connection.handle_msg_ref(&msg, forward);
        self.latency.handle_msg_ref(&msg, forward);
        self.config_file_dialog.handle_msg(msg, forward); // keep this last, eating up all the messages
    }
}

impl<T> eframe::App for Toplevel<T>
where
    T: StackType
        + HasNoteNames
        + PartialEq
        + Hash
        + Serialize
        + for<'a> Deserialize<'a>
        + Reloadable,
{
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.show_keyboard_controls && ui.button("hide controls").clicked() {
                    self.show_keyboard_controls = false;
                }
                if !self.show_keyboard_controls && ui.button("show controls").clicked() {
                    self.show_keyboard_controls = true;
                }

                self.connection_window
                    .show_hide_button(ui, "MIDI connections");
                self.note_window.show_hide_button(ui, "notes");

                if ui.button("save config").clicked() {
                    let gui_config = self.extract_config();
                    self.config_file_dialog.as_save().open(gui_config, &self.tx);
                }

                if ui.button("load config").clicked() {
                    let gui_config = self.extract_config();
                    self.config_file_dialog.as_load().open(gui_config, &self.tx);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                });
            });
        });

        egui::TopBottomPanel::bottom("small control bottom panel").show_animated(
            ctx,
            self.show_keyboard_controls,
            |ui| {
                AsKeyboardControls(&mut self.lattice).show(ui, &self.tx);
            },
        );

        egui::TopBottomPanel::top("strategy top panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                AsStrategyPicker(&mut self.strategies).show(ui, &self.tx);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.latency.show(ui, &self.tx);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            AsWindows(&mut self.strategies).show(ui, &self.state, &self.tx);
            if let Some(config) = self.config_file_dialog.show(ui) {
                let _ = T::initialise(&config.temperaments, &config.named_intervals);

                let time = Instant::now();
                let (process_config, gui_config, backend_config) = config.split();

                self.restart_from_config(gui_config, time);
                let _ = self.tx.send(FromUi::RestartProcessWithConfig {
                    config: process_config,
                    time,
                });
                let _ = self.tx.send(FromUi::RestartBackendWithConfig {
                    config: backend_config,
                    time,
                });

                return; // don't continue updating for this frame
            }
            self.lattice.show(ui, &self.state, &self.tx);
        });

        self.note_window.show("notes", ctx, |ui| {
            self.notes.show(ui, &self.tx);
        });

        self.connection_window.show("midi connections", ctx, |ui| {
            ui.vertical(|ui| {
                self.input_connection.show(ui, &self.tx);
                self.output_connection.show(ui, &self.tx);

                ui.separator();

                ui.vertical_centered(|ui| ui.label("output settings"));
                self.backend.show(ui, &self.tx);
            });
        });
    }
}

impl<T: StackType> ExtractConfig<GuiConfig<T>> for Toplevel<T> {
    fn extract_config(&self) -> GuiConfig<T> {
        let (strategies, tuning_editor, reference_editor) = self.strategies.extract_config();
        GuiConfig {
            strategies,
            lattice_window: self.lattice.extract_config(),
            backend_window: self.backend.extract_config(),
            latency_mean_over: self.latency.extract_config(),
            tuning_editor,
            reference_editor,
            use_cent_values: self
                .lattice
                .controls
                .correction_system_chooser
                .borrow()
                .extract_config(),
        }
    }
}
