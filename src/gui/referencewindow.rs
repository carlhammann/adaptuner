use std::sync::mpsc;

use eframe::egui;
use ndarray::Array1;

use crate::{
    interval::{
        base::Semitones,
        stack::{semitones_from_target, Stack},
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    msg::{FromUi, HandleMsg, ToUi},
    notename::{johnston::fivelimit::NoteName, NoteNameStyle},
    reference::{frequency_from_semitones, semitones_from_frequency, Reference},
};

use super::r#trait::GuiShow;

pub struct ReferenceWindow<T: StackType> {
    reference: Reference<T>,
    new_coeffs: Array1<StackCoeff>,
    new_semitones: Semitones,
    notenamestyle: NoteNameStyle,
}

impl<T: StackType> ReferenceWindow<T> {
    pub fn new(reference: Reference<T>, notenamestyle: NoteNameStyle) -> Self {
        Self {
            new_coeffs: reference.stack.target.clone(),
            new_semitones: reference.semitones,
            reference,
            notenamestyle,
        }
    }
}

impl<T: FiveLimitStackType> GuiShow<T> for ReferenceWindow<T> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        ui.label(format!(
            "Current reference is {} at {:.02} Hz (MIDI note {:.02}).",
            self.reference.stack.notename(&self.notenamestyle),
            self.reference.get_frequency(),
            self.reference.semitones
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
                "New reference is {} at",
                NoteName::new_from_coeffs::<T>(self.new_coeffs.view())
            ));

            let mut new_freq = frequency_from_semitones(self.new_semitones);
            ui.add(egui::DragValue::new(&mut new_freq));
            ui.label("Hz");
            self.new_semitones = semitones_from_frequency(new_freq);

            ui.label("(MIDI note");
            ui.add(egui::DragValue::new(&mut self.new_semitones));
            ui.label(").");
        });

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add(egui::Button::new("update current reference").selected(
                    (self.new_semitones != self.reference.semitones)
                        | (self.new_coeffs != self.reference.stack.target),
                ))
                .clicked()
            {
                self.reference = Reference::from_semitones(
                    Stack::from_target(self.new_coeffs.clone()),
                    self.new_semitones,
                );
                let _ = forward.send(FromUi::SetReference {
                    reference: self.reference.clone(),
                });
            }

            if ui.button("discard new reference").clicked() {
                self.new_coeffs.clone_from(&self.reference.stack.target);
                self.new_semitones = self.reference.semitones;
            }
        });
    }
}

impl<T: StackType> HandleMsg<ToUi<T>, FromUi<T>> for ReferenceWindow<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        todo!()
    }
}
