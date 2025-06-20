use std::sync::mpsc;

use eframe::{self, egui};

use crate::{
    // connections::{MidiInputOrConnection, MidiOutputOrConnection},
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackType},
    },
    msg::{FromUi, HandleMsg, HandleMsgRef, ToUi},
    neighbourhood::Neighbourhood,
    notename::NoteNameStyle,
    reference::Reference,
};

use super::{
    connectionwindow::{ConnectionWindow, Input, Output},
    latencywindow::LatencyWindow,
    latticewindow::LatticeWindow,
    notewindow::NoteWindow,
    r#trait::GuiShow,
    referencewindow::ReferenceWindow,
    tuningreferencewindow::TuningReferenceWindow,
};

pub struct ManyWindows<T: StackType, N: Neighbourhood<T>> {
    notewindow: NoteWindow<T>,
    latticewindow: LatticeWindow<T, N>,
    input_connection_window: ConnectionWindow<Input>,
    output_connection_window: ConnectionWindow<Output>,
    tuning_reference_window: TuningReferenceWindow<T>,
    reference_window: ReferenceWindow<T>,
    latencywindow: LatencyWindow,
    tx: mpsc::Sender<FromUi<T>>,
}

impl<T: FiveLimitStackType, N: Neighbourhood<T>> ManyWindows<T, N> {
    pub fn new(
        ctx: &egui::Context,
        latency_window_length: usize,
        tuning_reference: Reference<T>,
        reference: Stack<T>,
        considered_notes: N,
        notenamestyle: NoteNameStyle,
        tx: mpsc::Sender<FromUi<T>>,
    ) -> Self {
        Self {
            notewindow: NoteWindow::new(ctx),
            latticewindow: LatticeWindow::new(reference.clone(), considered_notes, 10.0),
            input_connection_window: ConnectionWindow::new(),
            output_connection_window: ConnectionWindow::new(),
            latencywindow: LatencyWindow::new(latency_window_length),
            tuning_reference_window: TuningReferenceWindow::new(tuning_reference, notenamestyle),
            reference_window: ReferenceWindow::new(reference, notenamestyle),
            tx,
        }
    }
}

impl<T: FiveLimitStackType, N: Neighbourhood<T>> HandleMsg<ToUi<T>, FromUi<T>>
    for ManyWindows<T, N>
{
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.notewindow.handle_msg_ref(&msg, forward);
        self.latticewindow.handle_msg_ref(&msg, forward);
        self.input_connection_window.handle_msg_ref(&msg, forward);
        self.output_connection_window.handle_msg_ref(&msg, forward);
        self.latencywindow.handle_msg_ref(&msg, forward);
    }
}

impl<T: FiveLimitStackType, N: Neighbourhood<T>> eframe::App for ManyWindows<T, N> {
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
            self.tuning_reference_window.show(ctx, ui, &self.tx);
        });
        
        egui::TopBottomPanel::bottom("reference").show(ctx, |ui| {
            self.reference_window.show(ctx, ui, &self.tx);
        });

        egui::CentralPanel::default().show(ctx, |_ui| {});

        egui::containers::Window::new("notes").show(ctx, |ui| {
            self.notewindow.show(ctx, ui, &self.tx);
        });

        egui::containers::Window::new("lattice").show(ctx, |ui| {
            self.latticewindow.show(ctx, ui, &self.tx);
        });
    }
}
