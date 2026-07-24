use std::time::Instant;

use eframe::egui;
use serde_derive::{Deserialize, Serialize};

use crate::{
    gui::{
        common::note_picker,
        r#trait::{GuiShow, UiAdaptor},
    },
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::FromUi,
    notename::{correction::Correction, HasNoteNames, NoteNameStyle},
    reference::{frequency_from_semitones, semitones_from_frequency, Reference},
};

pub struct TuningEditor<T: StackType> {
    new_reference: Reference<T>,
    temperaments_applied_to_new_reference: Vec<bool>,
    corrections_applied_to_new_reference: Correction<T>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct TuningEditorConfig {
    pub notenamestyle: NoteNameStyle,
}

impl<T: StackType> TuningEditor<T> {
    pub fn new() -> Self {
        Self {
            new_reference: Reference {
                stack: Stack::new_zero(),
                semitones: 60.0,
            },
            temperaments_applied_to_new_reference: vec![false; T::num_temperaments()],
            corrections_applied_to_new_reference: Correction::new_zero(),
        }
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for TuningEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        {
            let reference = adaptor.tuning_reference();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("Current tuning is ");
                ui.strong(
                    reference
                        .stack
                        .corrected_notename(&NoteNameStyle::Full, false),
                );
                ui.label(" at");
                ui.strong(format!(" {:.02} Hz", reference.get_frequency()));
                ui.label(format!(" (MIDI note {:.02})", reference.semitones));
            });
        }
        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        note_picker(
            ui,
            &mut self.temperaments_applied_to_new_reference,
            &mut self.corrections_applied_to_new_reference,
            &mut self.new_reference.stack,
        );

        ui.separator();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("New tuning will be ");
            ui.strong(
                self.new_reference
                    .stack
                    .corrected_notename(&NoteNameStyle::Full, false),
            );
            ui.label(" at ");

            let mut new_freq = frequency_from_semitones(self.new_reference.semitones);
            ui.add(egui::DragValue::new(&mut new_freq));
            ui.label(" Hz");
            self.new_reference.semitones = semitones_from_frequency(new_freq);

            ui.label(" (MIDI note");
            ui.add(egui::DragValue::new(&mut self.new_reference.semitones));
            ui.label(")");
        });

        let changed = *adaptor.tuning_reference() != self.new_reference;

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add_enabled(changed, egui::Button::new("update tuning"))
                .clicked()
            {
                adaptor
                    .tuning_reference_mut()
                    .clone_from(&self.new_reference);
                let _ = adaptor.send(FromUi::UpdateTuningReference {
                    time: Instant::now(),
                });
            }
        });
    }
}
