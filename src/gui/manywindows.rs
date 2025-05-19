use std::{cell::Cell, sync::mpsc, time::Instant};

use eframe::{self, egui};

use crate::{
    gui::{
        connectionwindow::ConnectionWindow,
        latencywindow::LatencyWindow,
        notewindow::NoteWindow,
        r#trait::{GuiShow, GuiShowUpdating, GuiState},
    },
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg,
};

use super::connectionwindow::MidiConnections;

pub struct ManyWindows<T: StackType> {
    notewindow: NoteWindow<T>,
    connectionwindow: ConnectionWindow,
    midi_connections: Cell<MidiConnections>,
    latencywindow: LatencyWindow,
}

impl<T: FiveLimitStackType> ManyWindows<T> {
    pub fn new(
        ctx: &egui::Context,
        midi_connections: MidiConnections,
        latency_window_length: usize,
    ) -> Self {
        Self {
            notewindow: NoteWindow::new(ctx),
            connectionwindow: ConnectionWindow::new(),
            midi_connections: Cell::new(midi_connections),
            latencywindow: LatencyWindow::new(latency_window_length),
        }
    }
}

impl<T: FiveLimitStackType> GuiState<T> for ManyWindows<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        ctx: &egui::Context,
    ) {
        self.notewindow.handle_msg(time, msg, to_process, ctx);
        //self.connectionwindow.handle_msg(time, msg, to_process, ctx);
        self.latencywindow.handle_msg(time, msg, to_process, ctx);
    }
}

impl<T: FiveLimitStackType> eframe::App for ManyWindows<T> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.latencywindow.show(ctx, ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                })
            });
        });

        egui::CentralPanel::default().show(ctx, |_ui| {});

        egui::containers::Window::new("notes").show(ctx, |ui| {
            self.notewindow.show(ctx, ui);
        });

        egui::containers::Window::new("MIDI connections")
            .resizable(false)
            .show(ctx, |ui| {
                let old = self
                    .midi_connections
                    .replace(MidiConnections::empty_placeholder());
                let new = self.connectionwindow.show_updating(old, ctx, ui);
                self.midi_connections.set(new);
            });
    }
}
