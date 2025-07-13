use eframe::egui;
use num_rational::Ratio;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    notename::correction::fivelimit::{Correction, CorrectionBasis},
};

pub fn correction_basis_chooser(ui: &mut egui::Ui, basis: &mut CorrectionBasis) {
    ui.vertical(|ui| {
        ui.label("show deviations as:");
        ui.selectable_value(basis, CorrectionBasis::Semitones, "cent values");
        ui.selectable_value(
            basis,
            CorrectionBasis::DiesisSyntonic,
            "diesis and syntonic comma",
        );
        ui.selectable_value(
            basis,
            CorrectionBasis::PythagoreanDiesis,
            "diesis and pythagorean comma",
        );
        ui.selectable_value(
            basis,
            CorrectionBasis::PythagoreanSyntonic,
            "syntonic and pyhtagorean commas",
        );
    });
}

/// returns true iff the number changed
pub fn rational_drag_value(ui: &mut egui::Ui, id: egui::Id, value: &mut Ratio<StackCoeff>) -> bool {
    let numer_id = id.with("numer");
    let denom_id = id.with("denom");

    let mut numer = ui
        .data(|map| map.get_temp(numer_id))
        .unwrap_or(*value.numer());
    let mut denom = ui
        .data(|map| map.get_temp(denom_id))
        .unwrap_or(*value.denom());

    let numer_response = ui.add(egui::DragValue::new(&mut numer));
    if numer_response.changed() {
        ui.data_mut(|map| map.insert_temp(numer_id, numer));
    }
    ui.label("/");
    let denom_response = ui.add(egui::DragValue::new(&mut denom));
    if denom_response.changed() {
        ui.data_mut(|map| map.insert_temp(denom_id, denom));
    }

    let finished = |r: &egui::Response| r.lost_focus() | r.drag_stopped();
    let started = |r: &egui::Response| r.gained_focus() | r.drag_started();

    if (finished(&denom_response) & !started(&numer_response))
        | (finished(&numer_response) & !started(&denom_response))
    {
        let new_numer = ui
            .data_mut(|map| map.remove_temp(numer_id))
            .unwrap_or(*value.numer());
        let new_denom = ui
            .data_mut(|map| map.remove_temp(denom_id))
            .unwrap_or(*value.denom());

        let new_value = Ratio::new(new_numer, new_denom.max(1));
        if new_value != *value {
            value.clone_from(&new_value);
            return true;
        }
    }

    false
}

pub fn note_picker<T: FiveLimitStackType>(
    ui: &mut egui::Ui,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction,
    correction_basis: &CorrectionBasis,
    stack: &mut Stack<T>,
) {
    ui.vertical(|ui| {
        let mut target_changed = false;
        ui.horizontal(|ui| {
            for (i, c) in stack.target.iter_mut().enumerate() {
                ui.label(format!("{}:", T::intervals()[i].name));
                if ui.add(egui::DragValue::new(c)).changed() {
                    target_changed = true;
                }
            }
        });

        if target_changed {
            tmp_temperaments.iter_mut().for_each(|b| *b = false);
            tmp_correction.reset_to_zero();
            stack.make_pure();
        }

        ui.label("tempered with:");

        temperament_applier(
            ui,
            tmp_temperaments,
            tmp_correction,
            correction_basis,
            stack,
        );
    });
}

pub fn temperament_applier<T: FiveLimitStackType>(
    ui: &mut egui::Ui,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction,
    correction_basis: &CorrectionBasis,
    stack: &mut Stack<T>,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for (i, t) in T::temperaments().iter().enumerate() {
                if ui.checkbox(&mut tmp_temperaments[i], &t.name).clicked() {
                    stack.retemper(tmp_temperaments);
                    *tmp_correction = Correction::new(stack);
                }
            }
        });

        let mut draw_corrections = true;
        let mut column = 0;
        match correction_basis {
            CorrectionBasis::Semitones => draw_corrections = false,
            CorrectionBasis::DiesisSyntonic => {}
            CorrectionBasis::PythagoreanSyntonic => column = 1,
            CorrectionBasis::PythagoreanDiesis => column = 2,
        }
        if draw_corrections {
            let mut changed = false;
            tmp_correction.mutate(correction_basis, |c1, c2| {
                ui.separator();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if rational_drag_value(ui, ui.id().with("c1"), c1) {
                            // if rational_number(ui, c1) {
                            changed = true;
                        }
                        ui.label(if column == 0 {
                            "diesis"
                        } else {
                            "pythagorean comma"
                        });
                    });
                    ui.horizontal(|ui| {
                        if rational_drag_value(ui, ui.id().with("c2"), c2) {
                            // if rational_number(ui, c2) {
                            changed = true;
                        }
                        ui.label(if column == 2 {
                            "diesis"
                        } else {
                            "syntonic comma"
                        });
                    });
                });
            });
            if changed {
                tmp_temperaments.iter_mut().for_each(|b| *b = false);
                stack.apply_correction(tmp_correction, correction_basis);
            }
        }
    });
}
