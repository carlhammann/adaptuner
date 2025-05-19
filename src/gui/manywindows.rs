use std::{cell::Cell, sync::mpsc, time::Instant};

use eframe::{self, egui};
use midir::{MidiInput, MidiOutput};

use crate::{
    connections::{MidiInputOrConnection, MidiOutputOrConnection},
    gui::{
        connectionwindow::{
            new_input_connection_window, new_output_connection_window, ConnectionWindow,
        },
        latencywindow::LatencyWindow,
        notewindow::NoteWindow,
        r#trait::{GuiShow, GuiShowUpdating, GuiState},
    },
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg,
};

pub struct ManyWindows<T: StackType> {
    notewindow: NoteWindow<T>,
    input_connection_window: ConnectionWindow<MidiInput>,
    midi_input: Cell<MidiInputOrConnection>,
    output_connection_window: ConnectionWindow<MidiOutput>,
    midi_output: Cell<MidiOutputOrConnection>,
    latencywindow: LatencyWindow,
}

impl<T: FiveLimitStackType> ManyWindows<T> {
    pub fn new(
        ctx: &egui::Context,
        midi_input: MidiInputOrConnection,
        midi_output: MidiOutputOrConnection,
        latency_window_length: usize,
    ) -> Self {
        Self {
            notewindow: NoteWindow::new(ctx),
            input_connection_window: new_input_connection_window(),
            midi_input: Cell::new(midi_input),
            output_connection_window: new_output_connection_window(),
            midi_output: Cell::new(midi_output),
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

        egui::TopBottomPanel::bottom("midi connections").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let old = self
                    .midi_input
                    .replace(MidiInputOrConnection::empty_placeholder());
                let new = self.input_connection_window.show_updating(old, ctx, ui);
                self.midi_input.set(new);

                let old = self
                    .midi_output
                    .replace(MidiOutputOrConnection::empty_placeholder());
                let new = self.output_connection_window.show_updating(old, ctx, ui);
                self.midi_output.set(new);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.notewindow.show(ctx, ui);
        });

        // egui::containers::Window::new("notes").show(ctx, |ui| {
        //     self.notewindow.show(ctx, ui);
        // });
    }
}
