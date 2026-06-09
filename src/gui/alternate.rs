use std::time::Instant;

use eframe::{self, egui};

use crate::{
    config::GuiConfig,
    gui::{
        backend::BackendWindow,
        common::SmallFloatingWindow,
        connection::{ConnectionWindow, Input, Output},
        latency::LatencyWindow,
        lattice::LatticeWindow,
        notifications::Notifications,
        r#trait::{Gui, GuiShow, UiAdaptor},
    },
    interval::stacktype::r#trait::StackType,
    msg::{ReceiveMsg, ReceiveMsgRef, ToUi},
    notename::HasNoteNames,
};

pub struct TopLevelGui<T: StackType, A: UiAdaptor<T>> {
    adaptor: A,

    input_connection: ConnectionWindow<Input>,
    output_connection: ConnectionWindow<Output>,
    backend: BackendWindow,
    connection_window: SmallFloatingWindow,

    notifications: Notifications<T>,

    show_side_panel: bool,

    lattice: LatticeWindow<T>,
    latency: LatencyWindow,
}

impl<T: StackType + HasNoteNames, A: UiAdaptor<T>> eframe::App for TopLevelGui<T, A> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // no need to check for the ConfigFileDialog, which is also shown as modal; this has its
        // own implementation of a modal window from egui_file_dialog
        let any_modal_open = false;
        // self.temperament_editor_window.is_open() || self.comma_editor_window.is_open();
        //

        egui::SidePanel::left("left panel").show_animated(ctx, self.show_side_panel, |ui| {
            if any_modal_open {
                ui.disable();
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.visuals_mut().collapsing_header_frame = true;

                // AsStrategyPicker(&mut self.strategies).show(ui, &self.state, &self.tx);
                //
                // ui.separator();

                self.connection_window
                    .show_hide_button(ui, "MIDI connections");
                // self.note_window.show_hide_button(ui, "notes");
                // self.keyboard_control_window
                //     .show_hide_button(ui, "keyboard controls");
                // if self
                //     .temperament_editor_window
                //     .show_hide_button(ui, "temperaments")
                // {
                //     self.current_config = self.extract_config();
                //     self.temperament_editor = TemperamentEditor::new();
                // }
                // if self.comma_editor_window.show_hide_button(ui, "commas") {
                //     self.current_config = self.extract_config();
                //     self.comma_editor = CommaEditor::new();
                // }

                ui.separator();

                // if ui.button("save configuration").clicked() {
                //     self.current_config = self.extract_config();
                //     self.current_process_config = None {};
                //     self.current_backend_config = None {};
                //     let _ = self.tx.send(FromUi::GetCurrentProcessConfig);
                //     let _ = self.tx.send(FromUi::GetCurrentBackendConfig);
                //     self.config_file_dialog.as_save().open();
                // }
                //
                // if ui.button("load configuration").clicked() {
                //     self.current_config = self.extract_config();
                //     self.current_process_config = None {};
                //     self.current_backend_config = None {};
                //     let _ = self.tx.send(FromUi::GetCurrentProcessConfig);
                //     let _ = self.tx.send(FromUi::GetCurrentBackendConfig);
                //     self.config_file_dialog.as_load().open();
                // }
                //
                // ui.separator();

                // AsBigControls(&mut self.lattice).show(ui);
                //
                // ui.separator();

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.notifications.clear_old(Instant::now());
            if self.notifications.is_nonempty() {
                egui::Window::new("notification window")
                    .title_bar(false)
                    .resizable(false)
                    .interactable(false)
                    .fixed_pos(ui.max_rect().center_top())
                    .pivot(egui::Align2::CENTER_TOP)
                    .show(ui.ctx(), |ui| {
                        if any_modal_open {
                            ui.disable();
                        }
                        self.notifications.show(ui, &self.adaptor)
                    });
            }

            self.connection_window.show("midi connections", ctx, |ui| {
                ui.vertical(|ui| {
                    if any_modal_open {
                        ui.disable();
                    }
                    self.input_connection.show(ui, &self.adaptor);
                    self.output_connection.show(ui, &self.adaptor);

                    ui.separator();

                    ui.vertical_centered(|ui| ui.label("output settings"));
                    self.backend.show(ui, &self.adaptor);
                });
            });

            self.lattice.show(ui, &self.adaptor);

            ui.horizontal(|ui| {
                if !self.show_side_panel {
                    if ui.button("☰").clicked() {
                        self.show_side_panel = true;
                    }
                } else {
                    if ui.button("⏴").clicked() {
                        self.show_side_panel = false;
                    }
                }

                if ui.button("🔍+").clicked() {
                    self.adaptor.config_mut().lattice.zoom *= 1.1;
                }
                if ui.button("🔍-").clicked() {
                    self.adaptor.config_mut().lattice.zoom /= 1.1;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    self.latency.show(ui);
                });
            });
        });
    }
}

impl<T: StackType, A: UiAdaptor<T>> ReceiveMsg<ToUi<T>> for TopLevelGui<T, A> {
    fn receive_msg(&mut self, msg: ToUi<T>) {
        self.lattice.receive_msg_ref(&msg);
        self.latency.receive_msg_ref(&msg);
        self.input_connection.receive_msg_ref(&msg);
        self.output_connection.receive_msg_ref(&msg);
        self.notifications.receive_msg_ref(&msg);
    }
}

impl<T: StackType + HasNoteNames, A: UiAdaptor<T>> Gui<T, A> for TopLevelGui<T, A> {
    fn new(config: GuiConfig, adaptor: A) -> Self {
        Self {
            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            backend: BackendWindow::new(adaptor.backend_config()),
            connection_window: SmallFloatingWindow::new(egui::Id::new("connection_window"), true),
            notifications: Notifications::new(),

            show_side_panel: false,

            lattice: LatticeWindow::new(),
            latency: LatencyWindow::new(config.latency_mean_over),

            adaptor,
        }
    }
}
