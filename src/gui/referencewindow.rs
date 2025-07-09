use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{
        correction::fivelimit::{Correction, CorrectionBasis},
        NoteNameStyle,
    },
};

use super::{common::correction_basis_chooser, r#trait::GuiShow};

pub struct ReferenceWindow<T: StackType> {
    new_reference: Stack<T>,
    reference: Stack<T>,
    applied_temperaments: Vec<bool>,
    notenamestyle: NoteNameStyle,
    correction_basis: CorrectionBasis,
}

pub struct ReferenceWindowConfig<T: StackType> {
    pub reference: Stack<T>,
    pub applied_temperaments: Vec<bool>,
    pub notenamestyle: NoteNameStyle,
}

impl<T: StackType> ReferenceWindow<T> {
    pub fn new(config: ReferenceWindowConfig<T>) -> Self {
        Self {
            new_reference: config.reference.clone(),
            reference: config.reference,
            applied_temperaments: config.applied_temperaments,
            notenamestyle: config.notenamestyle,
            correction_basis: CorrectionBasis::DiesisSyntonic,
        }
    }
}

impl<T: FiveLimitStackType + PartialEq> GuiShow<T> for ReferenceWindow<T> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        ui.label(format!(
            "Current reference is {}   {}{}",
            self.reference.notename(&self.notenamestyle),
            Correction::new(&self.reference, self.correction_basis),
            if self.reference.is_pure() & !self.reference.is_target() {
                format!(" = {}", self.reference.actual_notename(&self.notenamestyle))
            } else {
                "".into()
            }
        ));

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        ui.horizontal(|ui| {
            for (i, c) in self.new_reference.target.iter_mut().enumerate() {
                ui.label(format!("{}s:", T::intervals()[i].name));
                ui.add(egui::DragValue::new(c));
            }
        });
        for (i, t) in T::temperaments().iter().enumerate() {
            ui.checkbox(&mut self.applied_temperaments[i], &t.name);
        }

        self.new_reference.retemper(&self.applied_temperaments);

        ui.separator();

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label(format!(
                "New reference will be {}   {}{}",
                self.new_reference.notename(&self.notenamestyle),
                Correction::new(&self.new_reference, self.correction_basis),
                if self.new_reference.is_pure() & !self.new_reference.is_target() {
                    format!(
                        " = {}",
                        self.new_reference.actual_notename(&self.notenamestyle)
                    )
                } else {
                    "".into()
                }
            ));
        });

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add(
                    egui::Button::new("update reference")
                        .selected(self.new_reference != self.reference),
                )
                .clicked()
            {
                self.reference.clone_from(&self.new_reference);
                let _ = forward.send(FromUi::SetReference {
                    reference: self.reference.clone(),
                    time: Instant::now(),
                });
            }

            if ui.button("discard new reference").clicked() {
                self.new_reference.clone_from(&self.reference);
            }
        });

        ui.separator();

        correction_basis_chooser(ui, &mut self.correction_basis);
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ReferenceWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SetReference { stack } => {
                self.reference.clone_from(stack);
            }
            _ => {}
        }
    }
}
