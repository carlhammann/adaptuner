use std::{sync::mpsc, time::Instant};

use eframe::egui;
use ndarray::Array1;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    msg::{FromUi, HandleMsg, ToUi},
    notename::{johnston::fivelimit::NoteName, NoteNameStyle},
};

use super::r#trait::GuiShow;

pub struct ReferenceWindow<T: StackType> {
    reference: Stack<T>,
    new_coeffs: Array1<StackCoeff>,
    notenamestyle: NoteNameStyle,
}

impl<T: StackType> ReferenceWindow<T> {
    pub fn new(reference: Stack<T>, notenamestyle: NoteNameStyle) -> Self {
        Self {
            new_coeffs: reference.target.clone(),
            reference,
            notenamestyle,
        }
    }
}

impl<T: FiveLimitStackType> GuiShow<T> for ReferenceWindow<T> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        ui.label(format!(
            "Current reference is {}",
            self.reference.notename(&self.notenamestyle),
        ));

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            for (i, c) in self.new_coeffs.iter_mut().enumerate() {
                ui.label(format!("{}s:", T::intervals()[i].name));
                ui.add(egui::DragValue::new(c));
            }
        });

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label(format!(
                "New reference is {}",
                NoteName::new_from_values(
                    self.new_coeffs[T::octave_index()],
                    self.new_coeffs[T::fifth_index()],
                    self.new_coeffs[T::third_index()],
                )
            ));
        });

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add(
                    egui::Button::new("update current reference")
                        .selected(self.new_coeffs != self.reference.target),
                )
                .clicked()
            {
                self.reference = Stack::from_target(self.new_coeffs.clone());
                let _ = forward.send(FromUi::SetReference {
                    reference: self.reference.clone(),
                    time: Instant::now(),
                });
            }

            if ui.button("discard new reference").clicked() {
                self.new_coeffs.clone_from(&self.reference.target);
            }
        });
    }
}

impl<T: StackType> HandleMsg<ToUi<T>, FromUi<T>> for ReferenceWindow<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        todo!()
    }
}
