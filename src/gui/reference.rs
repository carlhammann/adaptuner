use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{correction::Correction, NoteNameStyle},
};

use super::{
    common::{correction_system_chooser, note_picker},
    r#trait::GuiShow,
};

pub struct ReferenceWindow<T: StackType> {
    reference: Option<Stack<T>>,
    new_reference: Stack<T>,
    temperaments_applied_to_new_reference: Vec<bool>,
    corrections_applied_to_new_reference: Correction<T>,
    notenamestyle: NoteNameStyle,
    correction_system_index: usize,
}

pub struct ReferenceWindowConfig {
    pub notenamestyle: NoteNameStyle,
    pub correction_system_index: usize,
}

impl<T: StackType> ReferenceWindow<T> {
    pub fn new(config: ReferenceWindowConfig) -> Self {
        Self {
            reference: None {},
            new_reference: Stack::new_zero(),
            temperaments_applied_to_new_reference: vec![false; T::num_temperaments()],
            corrections_applied_to_new_reference: Correction::new_zero(
                config.correction_system_index,
            ),
            notenamestyle: config.notenamestyle,
            correction_system_index: config.correction_system_index,
        }
    }
}

impl<T: FiveLimitStackType + PartialEq> GuiShow<T> for ReferenceWindow<T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(reference) = &self.reference {
            ui.label(format!(
                "Current reference is {}",
                reference.corrected_notename(&self.notenamestyle, self.correction_system_index),
            ));
        } else {
            ui.label("Currently no reference");
        }

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        note_picker(
            ui,
            &mut self.temperaments_applied_to_new_reference,
            &mut self.corrections_applied_to_new_reference,
            self.correction_system_index,
            &mut self.new_reference,
        );

        ui.separator();

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label(format!(
                "New reference will be {}",
                self.new_reference
                    .corrected_notename(&self.notenamestyle, self.correction_system_index),
            ));
        });

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
                let _ = forward.send(FromUi::SetReference {
                    reference: self.new_reference.clone(),
                    time: Instant::now(),
                });
            }
        });

        ui.separator();

        correction_system_chooser::<T>(ui, &mut self.correction_system_index);
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ReferenceWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SetReference { stack } => {
                self.reference = Some(stack.clone());
            }
            _ => {}
        }
    }
}
