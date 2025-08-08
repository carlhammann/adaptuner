use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui;
use serde_derive::{Deserialize, Serialize};

use crate::{
    config::ExtractConfig, gui::{
        common::{note_picker, CorrectionSystemChooser},
        r#trait::GuiShow,
    }, interval::{stack::Stack, stacktype::r#trait::StackType}, msg::{FromUi, ReceiveMsgRef, ToUi}, notename::{correction::Correction, HasNoteNames, NoteNameStyle}
};

pub struct ReferenceEditor<T: StackType> {
    reference: Option<Stack<T>>,
    new_reference: Stack<T>,
    temperaments_applied_to_new_reference: Vec<bool>,
    corrections_applied_to_new_reference: Correction<T>,
    notenamestyle: NoteNameStyle,
    correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ReferenceEditorConfig {
    pub notenamestyle: NoteNameStyle,
}

impl<T: StackType> ReferenceEditor<T> {
    pub fn new(
        config: ReferenceEditorConfig,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    ) -> Self {
        Self {
            reference: None {},
            new_reference: Stack::new_zero(),
            temperaments_applied_to_new_reference: vec![false; T::num_temperaments()],
            corrections_applied_to_new_reference: Correction::new_zero(),
            notenamestyle: config.notenamestyle,
            correction_system_chooser,
        }
    }
}

impl<T: StackType + HasNoteNames + PartialEq> GuiShow<T> for ReferenceEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(reference) = &self.reference {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("Current reference is ");
                ui.strong(reference.corrected_notename(
                    &self.notenamestyle,
                    self.correction_system_chooser.borrow().preference_order(),
                    self.correction_system_chooser.borrow().use_cent_values,
                ));
            });
        } else {
            ui.label("Currently no reference");
        }

        ui.separator();
        ui.label("Select new reference, relative to C 4:");
        note_picker(
            ui,
            &mut self.temperaments_applied_to_new_reference,
            &mut self.corrections_applied_to_new_reference,
            &mut self.new_reference,
            self.correction_system_chooser.borrow().preference_order(),
        );

        ui.separator();

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("New reference will be ");
                ui.strong(self.new_reference.corrected_notename(
                    &self.notenamestyle,
                    self.correction_system_chooser.borrow().preference_order(),
                    self.correction_system_chooser.borrow().use_cent_values,
                ));
            });
        });

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if ui
                .add_enabled(
                    match &self.reference {
                        None {} => true,
                        Some(r) => *r != self.new_reference,
                    },
                    egui::Button::new("update reference"),
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
    }
}

impl<T: StackType> ReceiveMsgRef<ToUi<T>> for ReferenceEditor<T> {
    fn receive_msg_ref(&mut self, msg: &ToUi<T>) {
        match msg {
            ToUi::SetReference { stack } => {
                self.reference = Some(stack.clone());
            }
            _ => {}
        }
    }
}

impl<T: StackType> ExtractConfig<ReferenceEditorConfig> for ReferenceEditor<T> {
    fn extract_config(&self) -> ReferenceEditorConfig {
        ReferenceEditorConfig {
            notenamestyle: self.notenamestyle,
        }
    }
}
