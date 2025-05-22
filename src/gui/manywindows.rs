use std::sync::mpsc;

use eframe::{self, egui};

use crate::{
    // connections::{MidiInputOrConnection, MidiOutputOrConnection},
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
    notename::NoteNameStyle,
    reference::Reference,
};

use super::{
    connectionwindow::{ConnectionWindow, Input, Output},
    latencywindow::LatencyWindow,
    notewindow::NoteWindow,
    r#trait::GuiShow,
    referencewindow::ReferenceWindow,
};

pub struct ManyWindows<T: StackType> {
    notewindow: NoteWindow<T>,
    input_connection_window: ConnectionWindow<Input>,
    output_connection_window: ConnectionWindow<Output>,
    reference_window: ReferenceWindow<T>,
    latencywindow: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,
}

impl<T: FiveLimitStackType> ManyWindows<T> {
    pub fn new(
        ctx: &egui::Context,
        latency_window_length: usize,
        reference: Reference<T>,
        notenamestyle: NoteNameStyle,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        Self {
            notewindow: NoteWindow::new(ctx),
            input_connection_window: ConnectionWindow::new(),
            output_connection_window: ConnectionWindow::new(),
            latencywindow: LatencyWindow::new(latency_window_length),
            reference_window: ReferenceWindow::new(reference, notenamestyle),
            tx,
        }
    }
}

impl<T: FiveLimitStackType> HandleMsg<ToUi<T>, FromUi<T>> for ManyWindows<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.notewindow.handle_msg_ref(&msg, forward);
        self.input_connection_window.handle_msg_ref(&msg, forward);
        self.output_connection_window.handle_msg_ref(&msg, forward);
        self.latencywindow.handle_msg_ref(&msg, forward);
    }
}

impl<T: FiveLimitStackType> eframe::App for ManyWindows<T> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.latencywindow.show(ctx, ui, &self.tx);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                })
            });
        });

        egui::TopBottomPanel::bottom("midi connections").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.input_connection_window.show(ctx, ui, &self.tx);
                self.output_connection_window.show(ctx, ui, &self.tx);
            });
        });

        egui::TopBottomPanel::bottom("global tuning reference").show(ctx, |ui| {
            self.reference_window.show(ctx, ui, &self.tx);
        });

        egui::CentralPanel::default().show(ctx, |_ui| {});

        egui::containers::Window::new("notes").show(ctx, |ui| {
            self.notewindow.show(ctx, ui, &self.tx);
        });
    }
}
