use eframe::egui;

use crate::{
    gui::{
        common::note_picker,
        r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    },
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg::ToUi,
    notename::{HasNoteNames, NoteNameStyle, correction::Correction},
    util::ordered_locks::Zero,
};

pub struct ReferenceEditor<T: StackType> {
    new_reference: Stack<T>,
    temperaments_applied_to_new_reference: Vec<bool>,
    corrections_applied_to_new_reference: Correction<T>,
}

impl<T: StackType> ReferenceEditor<T> {
    pub fn new() -> Self {
        Self {
            new_reference: Stack::new_zero(),
            temperaments_applied_to_new_reference: vec![false; T::num_temperaments()],
            corrections_applied_to_new_reference: Correction::new_zero(),
        }
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for ReferenceEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.initial_scale_reference_mut(|m_reference, adaptor| {
            if let Some(reference) = m_reference {
                ui.collapsing("initial scale reference", |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label("Currently, the initial scale reference is ");
                        ui.strong(reference.corrected_notename(
                            &NoteNameStyle::Full,
                            adaptor.config().use_cent_values,
                        ));
                    });

                    ui.separator();
                    ui.label("Select new reference, relative to C 4:");
                    note_picker(
                        ui,
                        &mut self.temperaments_applied_to_new_reference,
                        &mut self.corrections_applied_to_new_reference,
                        &mut self.new_reference,
                    );

                    ui.separator();

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label("Set initial scale reference to ");
                            ui.strong(self.new_reference.corrected_notename(
                                &NoteNameStyle::Full,
                                adaptor.config().use_cent_values,
                            ));
                        });
                    });

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        if ui
                            .add_enabled(
                                *reference != self.new_reference,
                                egui::Button::new("apply"),
                            )
                            .clicked()
                        {
                            reference.clone_from(&self.new_reference);
                        }
                    });
                });
            }
        });
        adaptor
    }
}

impl<T: StackType> ReceiveToUiRef<T> for ReferenceEditor<T> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        mut adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
        match msg {
            ToUi::UpdateReference {} => {
                (_, adaptor) = adaptor.reference(|r, _| {
                    self.new_reference.clone_from(r);
                });
            }
            _ => {}
        }
        adaptor
    }
}
