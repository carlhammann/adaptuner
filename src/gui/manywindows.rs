use std::{cell::RefCell, hash::Hash, rc::Rc, sync::mpsc};

use eframe::{self, egui};

use crate::{
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
};

use super::{
    backendwindow::{BackendWindow, BackendWindowConfig},
    connectionwindow::{ConnectionWindow, Input, Output},
    latencywindow::LatencyWindow,
    latticecontrolwindow::{lattice_control_window, LatticeWindowSmallControls},
    latticewindow::{LatticeWindow, LatticeWindowControls},
    notewindow::NoteWindow,
    r#trait::GuiShow,
    referencewindow::{ReferenceWindow, ReferenceWindowConfig},
    staticcontrolwindow::StaticControlWindow,
    tuningreferencewindow::{TuningReferenceWindow, TuningReferenceWindowConfig},
};

pub struct ManyWindows<T: StackType> {
    latticewindow: LatticeWindow<T>,
    lattice_window_controls: Rc<RefCell<LatticeWindowControls>>,
    lattice_window_small_controls: LatticeWindowSmallControls,

    show_control_panel: bool,
    static_control_window: StaticControlWindow<T>,

    input_connection_window: ConnectionWindow<Input>,
    output_connection_window: ConnectionWindow<Output>,
    show_connection_window: bool,
    sostenuto_is_next_neigbourhood: bool,
    soft_pedal_is_set_reference: bool,

    backend_window: BackendWindow,

    tuning_reference_window: TuningReferenceWindow<T>,
    show_tuning_reference_window: bool,

    reference_window: ReferenceWindow<T>,
    show_reference_window: bool,

    latencywindow: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,

    note_window: NoteWindow<T>,
    show_note_window: bool,
}

impl<T: FiveLimitStackType + Hash + Eq> ManyWindows<T> {
    pub fn new(
        lattice_window_config: LatticeWindowControls,
        reference_window_config: ReferenceWindowConfig,
        backend_window_config: BackendWindowConfig,
        latency_window_length: usize,
        tuning_reference_window_config: TuningReferenceWindowConfig,
        ctx: &egui::Context,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        let lattice_window_controls = Rc::new(RefCell::new(lattice_window_config));

        Self {
            latticewindow: LatticeWindow::new(lattice_window_controls.clone()),
            lattice_window_small_controls: LatticeWindowSmallControls(
                lattice_window_controls.clone(),
            ),
            lattice_window_controls,
            show_control_panel: false,
            static_control_window: StaticControlWindow::new(),
            input_connection_window: ConnectionWindow::new(),
            output_connection_window: ConnectionWindow::new(),
            show_connection_window: false,
            sostenuto_is_next_neigbourhood: true,
            soft_pedal_is_set_reference: true,
            backend_window: BackendWindow::new(backend_window_config),
            tuning_reference_window: TuningReferenceWindow::new(tuning_reference_window_config),
            show_tuning_reference_window: false,
            reference_window: ReferenceWindow::new(reference_window_config),
            show_reference_window: false,
            latencywindow: LatencyWindow::new(latency_window_length),
            note_window: NoteWindow::new(ctx),
            show_note_window: false,
            tx,
        }
    }
}

impl<T: FiveLimitStackType> HandleMsg<ToUi<T>, FromUi<T>> for ManyWindows<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.latticewindow.handle_msg_ref(&msg, forward);
        self.lattice_window_small_controls
            .handle_msg_ref(&msg, forward);
        self.static_control_window.handle_msg_ref(&msg, forward);
        self.note_window.handle_msg_ref(&msg, forward);
        self.input_connection_window.handle_msg_ref(&msg, forward);
        self.output_connection_window.handle_msg_ref(&msg, forward);
        self.latencywindow.handle_msg_ref(&msg, forward);
        self.reference_window.handle_msg_ref(&msg, forward);
        self.tuning_reference_window.handle_msg_ref(&msg, forward);
    }
}

impl<T: FiveLimitStackType + Hash + Eq> eframe::App for ManyWindows<T> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.latencywindow.show(ui, &self.tx);

                ui.separator();

                ui.toggle_value(&mut self.show_control_panel, "controls");
                ui.toggle_value(&mut self.show_connection_window, "midi connections");
                ui.toggle_value(&mut self.show_tuning_reference_window, "global tuning");
                ui.toggle_value(&mut self.show_reference_window, "reference");
                ui.toggle_value(&mut self.show_note_window, "notes");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                    ui.separator();
                })
            });
        });

        if self.show_control_panel {
            egui::TopBottomPanel::bottom("control panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    lattice_control_window::<T>(ui, &self.lattice_window_controls);
                    ui.separator();
                    self.static_control_window.show(ui, &self.tx);
                });
            });
        }

        egui::TopBottomPanel::bottom("small lattice control panel").show(ctx, |ui| {
            self.lattice_window_small_controls.show(ui, &self.tx);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.latticewindow.show(ui, &self.tx);
        });

        if self.show_connection_window {
            egui::containers::Window::new("midi connections")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        self.input_connection_window.show(ui, &self.tx);
                        self.output_connection_window.show(ui, &self.tx);

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
                        self.backend_window.show(ui, &self.tx);
                    });
                });
        }

        if self.show_tuning_reference_window {
            egui::containers::Window::new("global tuning")
                .collapsible(false)
                .show(ctx, |ui| {
                    self.tuning_reference_window.show(ui, &self.tx);
                });
        }

        if self.show_reference_window {
            egui::containers::Window::new("reference")
                .collapsible(false)
                .show(ctx, |ui| {
                    self.reference_window.show(ui, &self.tx);
                });
        }

        if self.show_note_window {
            egui::containers::Window::new("notes")
                .collapsible(false)
                .title_bar(false)
                .show(ctx, |ui| {
                    self.note_window.show(ui, &self.tx);
                });
        }
    }
}
