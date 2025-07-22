use std::{hash::Hash, sync::mpsc};

use eframe::{self, egui};

use crate::{
    config::ExtendedStrategyConfig,
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
};

use super::{
    backend::{BackendWindow, BackendWindowConfig},
    common::{show_hide_button, SmallFloatingWindow},
    connection::{ConnectionWindow, Input, Output},
    editor::{reference::ReferenceEditorConfig, tuning::TuningEditorConfig},
    latency::LatencyWindow,
    lattice::{LatticeWindow, LatticeWindowControls},
    latticecontrol::{AsBigControls, AsSmallControls},
    notes::NoteWindow,
    r#trait::GuiShow,
    strategy::{AsStrategyPicker, AsWindows, StrategyWindows},
};

pub struct Toplevel<T: StackType + 'static> {
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
}

impl<T: FiveLimitStackType + Hash + Eq> Toplevel<T> {
    pub fn new(
        strategy_names_and_kinds: Vec<ExtendedStrategyConfig<T>>,
        templates: &'static [ExtendedStrategyConfig<T>],
        lattice_config: LatticeWindowControls,
        reference_editor: ReferenceEditorConfig,
        backend_config: BackendWindowConfig,
        latency_length: usize,
        tuning_editor: TuningEditorConfig,
        ctx: &egui::Context,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        Self {
            lattice: LatticeWindow::new(lattice_config),
            show_controls: 0,
            old_show_controls: 1,

            strategies: StrategyWindows::new(
                strategy_names_and_kinds,
                templates,
                tuning_editor,
                reference_editor,
            ),

            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            connection_window: SmallFloatingWindow::new(egui::Id::new("connection_window")),
            backend: BackendWindow::new(backend_config),
            latency: LatencyWindow::new(latency_length),
            notes: NoteWindow::new(ctx),
            show_notes: false,
            notes_to_foreground: false,
            tx,
        }
    }
}

impl<T: FiveLimitStackType + 'static> HandleMsg<ToUi<T>, FromUi<T>> for Toplevel<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.lattice.handle_msg_ref(&msg, forward);
        self.notes.handle_msg_ref(&msg, forward);
        self.strategies.handle_msg_ref(&msg, forward);
        self.input_connection.handle_msg_ref(&msg, forward);
        self.output_connection.handle_msg_ref(&msg, forward);
        self.latency.handle_msg_ref(&msg, forward);
    }
}

impl<T: FiveLimitStackType + Hash + Eq + 'static> eframe::App for Toplevel<T> {
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
            self.lattice.show(ui, &self.tx);
            AsWindows(&mut self.strategies).show(ui, &self.tx);
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
