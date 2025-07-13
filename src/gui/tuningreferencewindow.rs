use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, IntervalBasis, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{correction::fivelimit::CorrectionBasis, NoteNameStyle},
    reference::{frequency_from_semitones, semitones_from_frequency, Reference},
};

use super::{common::correction_basis_chooser, r#trait::GuiShow};

pub struct TuningReferenceWindow<T: IntervalBasis> {
    reference: Option<Reference<T>>,
    new_reference: Reference<T>,
    notenamestyle: NoteNameStyle,
    correction_basis: CorrectionBasis,
}

pub struct TuningReferenceWindowConfig {
    pub notenamestyle: NoteNameStyle,
    pub correction_basis: CorrectionBasis,
}

impl<T: StackType> TuningReferenceWindow<T> {
    pub fn new(config: TuningReferenceWindowConfig) -> Self {
        Self {
            reference: None {},
            new_reference: Reference {
                stack: Stack::new_zero(),
                semitones: 60.0,
            },
            notenamestyle: config.notenamestyle,
            correction_basis: config.correction_basis,
        }
    }
}

impl<T: FiveLimitStackType + PartialEq> GuiShow<T> for TuningReferenceWindow<T> {
    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(reference) = &self.reference {
            ui.label(format!(
                "Current tuning reference is {} at {:.02} Hz (MIDI note {:.02}).",
                reference
                    .stack
                    .corrected_name(&self.notenamestyle, &self.correction_basis),
                reference.get_frequency(),
                reference.semitones
            ));
        } else {
            ui.label("Currently no global tuning");
        }

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            for (i, c) in self.new_reference.stack.target.iter_mut().enumerate() {
                ui.label(format!("{}:", T::intervals()[i].name));
                ui.add(egui::DragValue::new(c));
            }
        });

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label(format!(
                "New reference is {} at",
                self.new_reference
                    .stack
                    .corrected_name(&self.notenamestyle, &self.correction_basis)
            ));

            let mut new_freq = frequency_from_semitones(self.new_reference.semitones);
            ui.add(egui::DragValue::new(&mut new_freq));
            ui.label("Hz");
            self.new_reference.semitones = semitones_from_frequency(new_freq);

            ui.label("(MIDI note");
            ui.add(egui::DragValue::new(&mut self.new_reference.semitones));
            ui.label(").");
        });

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add(
                    egui::Button::new("update reference").selected(match &self.reference {
                        None {} => true,
                        Some(r) => *r != self.new_reference,
                    }),
                )
                .clicked()
            {
                self.reference = Some(self.new_reference.clone());
                let _ = forward.send(FromUi::SetTuningReference {
                    reference: self.new_reference.clone(),
                    time: Instant::now(),
                });
            }
        });

        ui.separator();

        correction_basis_chooser(ui, &mut self.correction_basis);
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for TuningReferenceWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SetTuningReference { reference } => self.reference = Some(reference.clone()),
            _ => {}
        }
    }
}
