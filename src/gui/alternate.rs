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

    lattice: LatticeWindow<T>,
    latency: LatencyWindow,
}

impl<T: StackType + HasNoteNames, A: UiAdaptor<T>> eframe::App for TopLevelGui<T, A> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // no need to check for the ConfigFileDialog, which is also shown as modal; this has its
        // own implementation of a modal window from egui_file_dialog
        let any_modal_open = false;
        // self.temperament_editor_window.is_open() || self.comma_editor_window.is_open();

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
                // if !*show_side_panel {
                //     if ui.button("☰").clicked() {
                //         *show_side_panel = true;
                //     }
                // } else {
                //     if ui.button("⏴").clicked() {
                //         *show_side_panel = false;
                //     }
                // }
                //
                // if ui.button("🔍+").clicked() {
                //     adaptor.config().lattice.zoom *= 1.1;
                // }
                // if ui.button("🔍-").clicked() {
                //     adaptor.config().lattice.zoom /= 1.1;
                // }
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

            lattice: LatticeWindow::new(),
            latency: LatencyWindow::new(config.latency_mean_over),

            adaptor,
        }
    }
}
