use eframe::egui;

use crate::notename::correction::fivelimit::CorrectionBasis;

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
