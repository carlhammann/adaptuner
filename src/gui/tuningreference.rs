use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{correction::Correction, NoteNameStyle},
    reference::{frequency_from_semitones, semitones_from_frequency, Reference},
};

use super::{
    common::{correction_system_chooser, note_picker},
    r#trait::GuiShow,
};

pub struct TuningReferenceWindow<T: StackType> {
    reference: Option<Reference<T>>,
    new_reference: Reference<T>,
    temperaments_applied_to_new_reference: Vec<bool>,
    corrections_applied_to_new_reference: Correction<T>,
    notenamestyle: NoteNameStyle,
    correction_system_index: usize,
}

pub struct TuningReferenceWindowConfig {
    pub notenamestyle: NoteNameStyle,
    pub correction_system_index: usize,
}

impl<T: StackType> TuningReferenceWindow<T> {
    pub fn new(config: TuningReferenceWindowConfig) -> Self {
        Self {
            reference: None {},
            new_reference: Reference {
                stack: Stack::new_zero(),
                semitones: 60.0,
            },
            temperaments_applied_to_new_reference: vec![false; T::num_temperaments()],
            corrections_applied_to_new_reference: Correction::new_zero(
                config.correction_system_index,
            ),
            notenamestyle: config.notenamestyle,
            correction_system_index: config.correction_system_index,
        }
    }
}

impl<T: FiveLimitStackType + PartialEq> GuiShow<T> for TuningReferenceWindow<T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(reference) = &self.reference {
            ui.label(format!(
                "Current tuning reference is {} at {:.02} Hz (MIDI note {:.02}).",
                reference
                    .stack
                    .corrected_notename(&self.notenamestyle, self.correction_system_index),
                reference.get_frequency(),
                reference.semitones
            ));
        } else {
            ui.label("Currently no global tuning");
        }

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        note_picker(
            ui,
            &mut self.temperaments_applied_to_new_reference,
            &mut self.corrections_applied_to_new_reference,
            self.correction_system_index,
            &mut self.new_reference.stack,
        );

        ui.separator();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label(format!(
                "New reference will be {} at",
                self.new_reference
                    .stack
                    .corrected_notename(&self.notenamestyle, self.correction_system_index)
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
                    egui::Button::new("update tuning").selected(match &self.reference {
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

        correction_system_chooser::<T>(ui, &mut self.correction_system_index);
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
