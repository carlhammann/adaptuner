use eframe::egui;

use crate::{
    config::{MelodyHarmonyCoordinationConfig, StrategyConfig},
    gui::r#trait::{GuiShow, UiAdaptor},
    interval::stacktype::r#trait::StackType,
    util::ordered_locks::Zero,
};

pub struct TwoStepEditor {}

impl<T: StackType> GuiShow<T> for TwoStepEditor {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.active_strategy_mut(|mut config, _| match &mut config {
            StrategyConfig::TwoStep {
                melody_harmony_coordination: MelodyHarmonyCoordinationConfig { group_ms, reanchor },
                ..
            } => {
                ui.collapsing("melody/harmony coordination", |ui| {
                    ui.radio_value(
                        reanchor,
                        false,
                        "do not move the scale reference on chord matches",
                    );

                    ui.radio_value(
                        reanchor,
                        true,
                        "whenever a chord matches, move \
                         the scale reference to that chord's reference",
                    );

                    if *reanchor {
                        ui.horizontal(|ui| {
                            ui.label("Allow re-setting the chord's reference for up to");
                            ui.add(egui::DragValue::new(group_ms).range(0..=1000));
                            ui.label("ms.");
                        });
                        ui.label(
                            "(This is to accommodate for the fact that we \
                            don't press and release all keys at exactly the same time)",
                        );
                    }
                });
            }
            _ => {}
        });
        adaptor
    }
}

impl TwoStepEditor {
    pub fn new() -> Self {
        Self {}
    }
}
