use std::sync::mpsc;

use eframe::egui;

use crate::{
    config::{HarmonyStrategyNames, MelodyStrategyNames},
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
};

pub struct TwoStepEditor {}

impl TwoStepEditor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show<T: StackType>(
        &mut self,
        ui: &mut egui::Ui,
        harmony: &mut HarmonyStrategyNames<T>,
        melody: &mut MelodyStrategyNames,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        match (harmony, melody) {
            (
                HarmonyStrategyNames::ChordList { .. },
                MelodyStrategyNames::Neighbourhoods {
                    group_ms, fixed, ..
                },
            ) => {
                if ui
                    .radio_value(fixed, true, "do not move the reference on chord matches")
                    .clicked()
                {
                    let _ = forward.send(FromUi::ReanchorOnMatch { reanchor: !*fixed });
                }

                if ui
                    .radio_value(
                        fixed,
                        false,
                        "whenever a chord matches, move \
                         the current reference to that chord's reference",
                    )
                    .clicked()
                {
                    let _ = forward.send(FromUi::ReanchorOnMatch { reanchor: !*fixed });
                }

                if !*fixed {
                    ui.horizontal(|ui| {
                        ui.label("Allow re-setting the chord's reference for up to");
                        if ui
                            .add(egui::DragValue::new(group_ms).range(0..=1000))
                            .changed()
                        {
                            let _ = forward.send(FromUi::SetGroupMs {
                                group_ms: *group_ms,
                            });
                        }
                        ui.label("ms.");
                    });
                    ui.label(
                        "(This is to accommodate for the fact that we \
                            don't press and release all keys at exactly the same time)",
                    );
                }
            }
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for TwoStepEditor {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        // todo!()
    }
}
