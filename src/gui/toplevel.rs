use std::{cell::RefCell, hash::Hash, rc::Rc, sync::mpsc};

use eframe::{self, egui};

use crate::{
    config::StrategyKind,
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
};

use super::{
    backend::{BackendWindow, BackendWindowConfig},
    connection::{ConnectionWindow, Input, Output},
    editor::{reference::ReferenceEditorConfig, tuning::TuningEditorConfig},
    latency::LatencyWindow,
    lattice::{LatticeWindow, LatticeWindowControls},
    latticecontrol::{lattice_control, LatticeWindowSmallControls},
    notes::NoteWindow,
    r#trait::GuiShow,
    strategy::{AsStrategyPicker, AsWindows, StrategyWindows},
};

pub struct Toplevel<T: StackType> {
    lattice: LatticeWindow<T>,
    lattice_controls: Rc<RefCell<LatticeWindowControls>>,
    show_lattice_controls: bool,
    lattice_small_controls: LatticeWindowSmallControls,

    strategies: StrategyWindows<T>,

    input_connection: ConnectionWindow<Input>,
    output_connection: ConnectionWindow<Output>,
    show_connection: bool,
    sostenuto_is_next_neigbourhood: bool,
    soft_pedal_is_set_reference: bool,

    backend: BackendWindow,

    latency: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,

    notes: NoteWindow<T>,
    show_notes: bool,
}

impl<T: FiveLimitStackType + Hash + Eq> Toplevel<T> {
    pub fn new(
        strategy_names_and_kinds: Vec<(String, StrategyKind)>,
        lattice_config: LatticeWindowControls,
        reference_editor: ReferenceEditorConfig,
        backend_config: BackendWindowConfig,
        latency_length: usize,
        tuning_editor: TuningEditorConfig,
        ctx: &egui::Context,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        let lattice_controls = Rc::new(RefCell::new(lattice_config));

        Self {
            lattice: LatticeWindow::new(lattice_controls.clone()),
            lattice_small_controls: LatticeWindowSmallControls(lattice_controls.clone()),
            lattice_controls,
            show_lattice_controls: false,

            strategies: StrategyWindows::new(
                strategy_names_and_kinds,
                tuning_editor,
                reference_editor,
            ),

            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            show_connection: false,
            sostenuto_is_next_neigbourhood: true,
            soft_pedal_is_set_reference: true,
            backend: BackendWindow::new(backend_config),
            latency: LatencyWindow::new(latency_length),
            notes: NoteWindow::new(ctx),
            show_notes: false,
            tx,
        }
    }
}

impl<T: FiveLimitStackType> HandleMsg<ToUi<T>, FromUi<T>> for Toplevel<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.strategies.handle_msg_ref(&msg, forward);
        self.lattice.handle_msg_ref(&msg, forward);
        self.lattice_small_controls.handle_msg_ref(&msg, forward);
        self.notes.handle_msg_ref(&msg, forward);
        self.input_connection.handle_msg_ref(&msg, forward);
        self.output_connection.handle_msg_ref(&msg, forward);
        self.latency.handle_msg_ref(&msg, forward);
    }
}

impl<T: FiveLimitStackType + Hash + Eq> eframe::App for Toplevel<T> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.latency.show(ui, &self.tx);

                ui.separator();

                ui.toggle_value(&mut self.show_connection, "midi connections");
                ui.toggle_value(&mut self.show_notes, "notes");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                    ui.separator();
                })
            });
        });

        egui::TopBottomPanel::bottom("strategy picker panel").show(ctx, |ui| {
            AsStrategyPicker(&mut self.strategies).show(ui, &self.tx);
        });

        egui::TopBottomPanel::top("small lattice control panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.toggle_value(&mut self.show_lattice_controls, "more");
                self.lattice_small_controls.show(ui, &self.tx);
            });
        });

        if self.show_lattice_controls {
            egui::TopBottomPanel::top("lattice control panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    lattice_control::<T>(ui, &self.lattice_controls);
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.lattice.show(ui, &self.tx);
            AsWindows(&mut self.strategies).show(ui, &self.tx);
        });

        if self.show_notes {
            egui::containers::Window::new("notes")
                .open(&mut self.show_notes)
                .show(ctx, |ui| {
                    self.notes.show(ui, &self.tx);
                });
        }

        if self.show_connection {
            egui::containers::Window::new("midi connections")
                .open(&mut self.show_connection)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        self.input_connection.show(ui, &self.tx);
                        self.output_connection.show(ui, &self.tx);

                        ui.separator();

                        ui.vertical_centered(|ui| ui.label("input settings"));
                        if ui
                            .toggle_value(
                                &mut self.sostenuto_is_next_neigbourhood,
                                "use sostenuto pedal (middle) to switch neighbourhoods",
                            )
                            .clicked()
                        {
                            let _ = self.tx.send(FromUi::ToggleSostenutoIsNextNeighbourhood {});
                        };
                        if ui
                            .toggle_value(
                                &mut self.soft_pedal_is_set_reference,
                                "use soft pedal (left) to reset reference",
                            )
                            .clicked()
                        {
                            let _ = self.tx.send(FromUi::ToggleSoftPedalIsSetReference {});
                        }

                        ui.separator();

                        ui.vertical_centered(|ui| ui.label("output settings"));
                        self.backend.show(ui, &self.tx);
                    });
                });
        }
    }
}
