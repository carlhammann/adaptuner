use std::{hash::Hash, sync::mpsc};

use eframe::{self, egui};
use serde::{Deserialize, Serialize};

use crate::{
    bindable::{Bindable, Bindings},
    config::{ExtractConfig, GuiConfig, StrategyNames},
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
    notename::HasNoteNames,
};

use super::{
    backend::{BackendWindow, BackendWindowConfig},
    common::{show_hide_button, SmallFloatingWindow},
    config::save::ConfigSaver,
    connection::{ConnectionWindow, Input, Output},
    editor::{reference::ReferenceEditorConfig, tuning::TuningEditorConfig},
    latency::LatencyWindow,
    lattice::{LatticeWindow, LatticeWindowConfig},
    latticecontrol::{AsBigControls, AsSmallControls},
    notes::NoteWindow,
    r#trait::GuiShow,
    strategy::{AsStrategyPicker, AsWindows, StrategyWindows},
};

pub struct Toplevel<T: StackType> {
    lattice: LatticeWindow<T>,
    show_controls: u8,
    old_show_controls: u8,

    strategies: StrategyWindows<T>,

    input_connection: ConnectionWindow<Input>,
    output_connection: ConnectionWindow<Output>,
    connection_window: SmallFloatingWindow,

    backend: BackendWindow,

    latency: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,

    notes: NoteWindow<T>,
    show_notes: bool,
    notes_to_foreground: bool,

    config_saver: ConfigSaver<T>,
}

impl<T: StackType + HasNoteNames + Hash + Eq + Serialize> Toplevel<T> {
    pub fn new(
        strategy_names_and_bindings: Vec<(StrategyNames, Bindings<Bindable>)>,
        lattice_config: LatticeWindowConfig<T>,
        reference_editor: ReferenceEditorConfig,
        backend_config: BackendWindowConfig,
        latency_length: usize,
        tuning_editor: TuningEditorConfig,
        ctx: &egui::Context,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        let lattice_controls = lattice_config.to_controls();

        Self {
            strategies: StrategyWindows::new(
                strategy_names_and_bindings,
                tuning_editor,
                reference_editor,
                lattice_controls.correction_system_chooser.clone(),
            ),

            lattice: LatticeWindow::new(lattice_controls),
            show_controls: 0,
            old_show_controls: 1,

            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            connection_window: SmallFloatingWindow::new(egui::Id::new("connection_window")),
            backend: BackendWindow::new(backend_config),
            latency: LatencyWindow::new(latency_length),
            notes: NoteWindow::new(ctx),
            show_notes: false,
            notes_to_foreground: false,
            tx,
            config_saver: ConfigSaver::new(),
        }
    }
}

impl<T: StackType + Serialize> HandleMsg<ToUi<T>, FromUi<T>> for Toplevel<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.lattice.handle_msg_ref(&msg, forward);
        self.notes.handle_msg_ref(&msg, forward);
        self.strategies.handle_msg_ref(&msg, forward);
        self.input_connection.handle_msg_ref(&msg, forward);
        self.output_connection.handle_msg_ref(&msg, forward);
        self.latency.handle_msg_ref(&msg, forward);
        self.config_saver.handle_msg(msg, forward); // keep this last, eating up all the messages
    }
}

impl<T: StackType + HasNoteNames + PartialEq + Hash + Serialize + for<'a> Deserialize<'a>>
    eframe::App for Toplevel<T>
{
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.show_controls == 0 {
                    if ui.button("show controls").clicked() {
                        self.show_controls = 1;
                        self.old_show_controls = 0;
                    }
                } else if (self.show_controls == 1) & (self.old_show_controls == 0) {
                    if ui.button("more controls").clicked() {
                        self.show_controls = 2;
                        self.old_show_controls = 1;
                    }
                } else if (self.show_controls == 1) & (self.old_show_controls == 2) {
                    if ui.button("hide controls").clicked() {
                        self.show_controls = 0;
                        self.old_show_controls = 1;
                    }
                } else {
                    if ui.button("fewer controls").clicked() {
                        self.show_controls = 1;
                        self.old_show_controls = 2;
                    }
                }

                self.connection_window
                    .show_hide_button(ui, "MIDI connections");
                show_hide_button(
                    ui,
                    "notes",
                    &mut self.show_notes,
                    &mut self.notes_to_foreground,
                );

                if ui.button("save config").clicked() {
                    self.config_saver.open(self.extract_config(), &self.tx);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                });
            });
        });

        egui::TopBottomPanel::bottom("big control bottom panel").show_animated(
            ctx,
            self.show_controls > 1,
            |ui| {
                AsBigControls(&mut self.lattice).show(ui, &self.tx);
            },
        );

        egui::TopBottomPanel::bottom("small control bottom panel").show_animated(
            ctx,
            self.show_controls > 0,
            |ui| {
                AsSmallControls(&mut self.lattice).show(ui, &self.tx);
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
            AsWindows(&mut self.strategies).show(ui, &self.tx);
            self.config_saver.show(ui, &self.tx);
            self.lattice.show(ui, &self.tx);
        });

        let note_window_id = egui::Id::new("note_window_id");
        if self.show_notes {
            egui::containers::Window::new("notes")
                .id(note_window_id)
                .open(&mut self.show_notes)
                .show(ctx, |ui| {
                    self.notes.show(ui, &self.tx);
                });
        }
        if self.notes_to_foreground {
            let layer_id = egui::LayerId::new(egui::Order::Middle, note_window_id);
            ctx.move_to_top(layer_id);
            self.notes_to_foreground = false;
        }

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

impl<T: StackType> ExtractConfig<GuiConfig> for Toplevel<T> {
    fn extract_config(&self) -> GuiConfig {
        GuiConfig {
            strategies: self.strategies.strategies().into(),
        }
    }
}
