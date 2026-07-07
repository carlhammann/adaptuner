use std::time::Instant;

use eframe::{self, egui};
use serde::{Deserialize, Serialize};

use crate::{
    config::BackendConfig,
    gui::{
        backend::BackendWindow,
        common::SmallFloatingWindow,
        config::ConfigFileDialog,
        connection::{ConnectionWindow, Input, Output},
        editor::{commas::CommaEditor, temperament::TemperamentEditor},
        latency::LatencyWindow,
        lattice::LatticeWindow,
        notifications::Notifications,
        r#trait::{Gui, GuiShow, UiAdaptor},
    },
    interval::stacktype::r#trait::{Reloadable, StackType},
    msg::{FromUi, ReceiveMsg, ReceiveMsgRef, ToUi},
    notename::HasNoteNames,
};

pub struct TopLevelGui<T: StackType, A: UiAdaptor<T>> {
    adaptor: A,

    // these four use the same SmallFloatingWindow, namely the connection_window
    input_connection: ConnectionWindow<Input>,
    output_connection: ConnectionWindow<Output>,
    backend: BackendWindow,
    connection_window: SmallFloatingWindow,

    stopped: bool,

    config_file_dialog: ConfigFileDialog<T>,

    notifications: Notifications<T>,

    show_side_panel: bool,

    lattice: LatticeWindow<T>,
    latency: LatencyWindow,

    temperament_editor: TemperamentEditor<T>,
    temperament_editor_window: SmallFloatingWindow,

    comma_editor: CommaEditor<T>,
    comma_editor_window: SmallFloatingWindow,
}

impl<T, A> eframe::App for TopLevelGui<T, A>
where
    T: StackType + HasNoteNames + Serialize + for<'a> Deserialize<'a> + Reloadable,
    A: UiAdaptor<T>,
{
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // no need to check for the ConfigFileDialog, which is also shown as modal; this has its
        // own implementation of a modal window from egui_file_dialog
        let any_modal_open =
            self.temperament_editor_window.is_open() || self.comma_editor_window.is_open();

        let mut stop = |tlg: &mut TopLevelGui<T, A>| {
            let _ = tlg.adaptor.send(FromUi::Stop {
                time: Instant::now(),
            });
            tlg.stopped = true;
        };

        let mut restart = |tlg: &mut TopLevelGui<T, A>| {
            tlg.renew();
            let _ = tlg.adaptor.send(FromUi::RestartFromConfig {
                time: Instant::now(),
            });
            tlg.stopped = false;
        };

        // each of these windows stops all processes when it is opened, and sets self.stopped. If we
        // close the window without any changes, we'll want to restart.
        let restart_needed = self.stopped
            && !self.temperament_editor_window.is_open()
            && !self.comma_editor_window.is_open()
            && !self.config_file_dialog.is_open();

        if restart_needed {
            restart(self);
        }

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

                ui.separator();

                if self
                    .temperament_editor_window
                    .show_hide_button(ui, "temperaments")
                {
                    stop(self);
                    self.temperament_editor = TemperamentEditor::new();
                }
                if self.comma_editor_window.show_hide_button(ui, "commas") {
                    stop(self);
                    self.comma_editor = CommaEditor::new();
                }

                ui.separator();

                if ui.button("save configuration").clicked() {
                    self.config_file_dialog.as_save().open();
                }

                if ui.button("load configuration").clicked() {
                    stop(self);
                    self.config_file_dialog.as_load().open();
                }

                ui.separator();

                // AsBigControls(&mut self.lattice).show(ui);
                //
                // ui.separator();

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if any_modal_open {
                ui.disable();
            }
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

            let new_config = self.config_file_dialog.show(ui, &self.adaptor);
            if let Some(config) = new_config {
                let _ = T::initialise(config.temperaments, config.named_intervals);

                *self.adaptor.config_mut() = config.gui;
                match config.backend {
                    BackendConfig::Pitchbend12(c) => *self.adaptor.backend_config_mut() = c,
                }
                *self.adaptor.strategy_config_mut() = config.strategies;

                restart(self);

                return; // don't continue updating for this frame
            }

            if let Some(egui::InnerResponse {
                inner: Some(Some(new_temperament_definitions)),
                ..
            }) = self
                .temperament_editor_window
                .show("temperaments", ctx, |ui| self.temperament_editor.show(ui))
            {
                let named_intervals = T::named_intervals().clone();
                let _ = T::initialise(new_temperament_definitions, named_intervals);

                restart(self);

                return; // don't continue updating for this frame
            }

            if let Some(egui::InnerResponse {
                inner: Some(Some(new_named_intervals)),
                ..
            }) = self
                .comma_editor_window
                .show("commas", ctx, |ui| self.comma_editor.show(ui))
            {
                // If this window is open, we're loading new temperaments (i.e. a new StackType),
                // and we've already stopped the other processes.
                // At the end of this block, we'll restart everything.
                let temperament_definitions = T::temperament_definitions().clone();
                let _ = T::initialise(temperament_definitions, new_named_intervals);

                restart(self);

                return; // don't continue updating for this frame
            }

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

impl<T, A> Gui<T, A> for TopLevelGui<T, A>
where
    T: StackType + HasNoteNames + Serialize + for<'a> Deserialize<'a> + Reloadable,
    A: UiAdaptor<T>,
{
    fn new(adaptor: A) -> Self {
        let latency_mean_over = adaptor.config().latency_mean_over;
        Self {
            input_connection: ConnectionWindow::new(),
            output_connection: ConnectionWindow::new(),
            backend: BackendWindow::new(adaptor.backend_config()),
            connection_window: SmallFloatingWindow::new(egui::Id::new("connection_window"), true),
            notifications: Notifications::new(),

            stopped: false,

            config_file_dialog: ConfigFileDialog::new(),

            show_side_panel: false,

            lattice: LatticeWindow::new(),
            latency: LatencyWindow::new(latency_mean_over),

            temperament_editor: TemperamentEditor::new(),
            temperament_editor_window: SmallFloatingWindow::new(
                egui::Id::new("temperament_editor_window"),
                false,
            ),
            comma_editor: CommaEditor::new(),
            comma_editor_window: SmallFloatingWindow::new(
                egui::Id::new("comma_editor_window"),
                false,
            ),

            adaptor,
        }
    }
}

impl<T, A> TopLevelGui<T, A>
where
    T: StackType + HasNoteNames + Serialize + for<'a> Deserialize<'a> + Reloadable,
    A: UiAdaptor<T>,
{
    fn renew(&mut self) {
        self.backend = BackendWindow::new(self.adaptor.backend_config());
        self.notifications = Notifications::new();
        self.config_file_dialog = ConfigFileDialog::new();
        self.lattice = LatticeWindow::new();
        self.latency = LatencyWindow::new(self.adaptor.config().latency_mean_over);

        self.temperament_editor = TemperamentEditor::new();
        self.comma_editor = CommaEditor::new();
    }
}
