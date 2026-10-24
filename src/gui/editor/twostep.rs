use std::time::Instant;

use eframe::egui;

use crate::{
    config::{MelodyHarmonyCoordinationConfig, MelodyStrategyConfig, StrategyConfig},
    gui::r#trait::{GuiShow, UiAdaptor},
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, ToMelody, ToStaticNeighbourhoodsAsMelody, ToStrategy, ToTwoStep},
    strategy::melody::{
        neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        r#trait::{ChordAnchoringKind, UndeterminedSpringAnchoringKind},
    },
    util::ordered_locks::Zero,
};

pub struct TwoStepEditor {
    most_common_notes_fallback: Box<UndeterminedSpringAnchoringKind>,
    min_number_of_common_notes: usize,
    most_common_pitch_classes_fallback: Box<UndeterminedSpringAnchoringKind>,
    min_number_of_common_pitch_classes: usize,
}

impl<T: StackType> GuiShow<T> for TwoStepEditor {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.active_strategy_mut(|mut config, adaptor| match &mut config {
            StrategyConfig::TwoStep {
                melody_harmony_coordination:
                    MelodyHarmonyCoordinationConfig {
                        group_ms,
                        reanchor,
                        tune_wait_us,
                    },
                melody:
                    MelodyStrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig {
                        chord_anchoring_kind,
                        spring_anchoring_kind,
                        ..
                    }),
                ..
            } => {
                ui.collapsing("melody/harmony coordination", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Wait for");
                        ui.add(egui::DragValue::new(tune_wait_us).range(0..=1000000))
                            .on_hover_text_at_pointer(
                                "Some hardware and software instruments are \
                             overwhelmed by the flood of tuning information \
                             adaptuner can send. This setting gives them a bit of breathing \
                             room, at the cost of some added latency.",
                            );
                        ui.label("μs before using imperfect harmony solutions.");
                    });

                    let mut change_anchoring = false;
                    ui.collapsing("anchoring for defined chords", |ui| {
                        change_anchoring |= ui
                            .radio_value(
                                chord_anchoring_kind,
                                ChordAnchoringKind::ChordReference,
                                "defined reference note of chord",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                chord_anchoring_kind,
                                ChordAnchoringKind::LowestKey,
                                "lowest key",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                chord_anchoring_kind,
                                ChordAnchoringKind::HighestKey,
                                "highest key",
                            )
                            .changed();
                    });

                    ui.collapsing("anchoring for spring chords", |ui| {
                        change_anchoring |= ui
                            .radio_value(
                                spring_anchoring_kind,
                                UndeterminedSpringAnchoringKind::Fundamental,
                                "highest fundamental",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                spring_anchoring_kind,
                                UndeterminedSpringAnchoringKind::Overtone,
                                "lowest overtone",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                spring_anchoring_kind,
                                UndeterminedSpringAnchoringKind::FundamentalOrOvertone,
                                "fundamental or overtone, whichever is closer",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                spring_anchoring_kind,
                                UndeterminedSpringAnchoringKind::LowestKey,
                                "lowest key",
                            )
                            .changed();
                        change_anchoring |= ui
                            .radio_value(
                                spring_anchoring_kind,
                                UndeterminedSpringAnchoringKind::HighestKey,
                                "highest key",
                            )
                            .changed();

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if let UndeterminedSpringAnchoringKind::MostCommonNotes {
                                count_pitch_classes: false,
                                fallback,
                                min_number_of_common_notes,
                                ..
                            } = spring_anchoring_kind
                            {
                                self.most_common_notes_fallback.clone_from(fallback);
                                self.min_number_of_common_notes = *min_number_of_common_notes;
                            }
                            change_anchoring |= ui
                                .radio_value(
                                    spring_anchoring_kind,
                                    UndeterminedSpringAnchoringKind::MostCommonNotes {
                                        min_number_of_common_notes: self.min_number_of_common_notes,
                                        count_pitch_classes: false,
                                        fallback: self.most_common_notes_fallback.clone(),
                                    },
                                    "most notes from current scale",
                                )
                                .changed();
                        });
                        if let UndeterminedSpringAnchoringKind::MostCommonNotes {
                            count_pitch_classes: false,
                            fallback,
                            min_number_of_common_notes,
                        } = spring_anchoring_kind
                        {
                            ui.horizontal(|ui| {
                                ui.label(" (at least ");
                                change_anchoring |= ui
                                    .add(
                                        egui::DragValue::new(min_number_of_common_notes)
                                            .range(2..=128),
                                    )
                                    .changed();
                                ui.label(") or ");
                            });
                            egui::ComboBox::from_id_salt(
                                "alternative spring anchor for most common notes",
                            )
                            .selected_text(format!("{}", self.most_common_notes_fallback))
                            .show_ui(ui, |ui| {
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::Fundamental),
                                        format!("{}", UndeterminedSpringAnchoringKind::Fundamental),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::Overtone),
                                        format!("{}", UndeterminedSpringAnchoringKind::Overtone),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(
                                            UndeterminedSpringAnchoringKind::FundamentalOrOvertone,
                                        ),
                                        format!(
                                            "{}",
                                            UndeterminedSpringAnchoringKind::FundamentalOrOvertone
                                        ),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::LowestKey),
                                        format!("{}", UndeterminedSpringAnchoringKind::LowestKey),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::HighestKey),
                                        format!("{}", UndeterminedSpringAnchoringKind::HighestKey),
                                    )
                                    .changed();
                            });
                        }

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if let UndeterminedSpringAnchoringKind::MostCommonNotes {
                                count_pitch_classes: true,
                                fallback,
                                min_number_of_common_notes,
                                ..
                            } = spring_anchoring_kind
                            {
                                self.most_common_pitch_classes_fallback.clone_from(fallback);
                                self.min_number_of_common_pitch_classes =
                                    *min_number_of_common_notes;
                            }
                            change_anchoring |= ui
                                .radio_value(
                                    spring_anchoring_kind,
                                    UndeterminedSpringAnchoringKind::MostCommonNotes {
                                        count_pitch_classes: true,
                                        fallback: self.most_common_pitch_classes_fallback.clone(),
                                        min_number_of_common_notes: self
                                            .min_number_of_common_pitch_classes,
                                    },
                                    "most pitch classes from current scale",
                                )
                                .changed();
                        });
                        if let UndeterminedSpringAnchoringKind::MostCommonNotes {
                            count_pitch_classes: true,
                            fallback,
                            min_number_of_common_notes,
                        } = spring_anchoring_kind
                        {
                            ui.horizontal(|ui| {
                                ui.label(" (at least ");
                                change_anchoring |= ui
                                    .add(
                                        egui::DragValue::new(min_number_of_common_notes)
                                            .range(2..=128),
                                    )
                                    .changed();
                                ui.label(") or ");
                            });
                            egui::ComboBox::from_id_salt(
                                "alternative spring anchor for most common pitch classes",
                            )
                            .selected_text(format!("{}", self.most_common_pitch_classes_fallback))
                            .show_ui(ui, |ui| {
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::Fundamental),
                                        format!("{}", UndeterminedSpringAnchoringKind::Fundamental),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::Overtone),
                                        format!("{}", UndeterminedSpringAnchoringKind::Overtone),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(
                                            UndeterminedSpringAnchoringKind::FundamentalOrOvertone,
                                        ),
                                        format!(
                                            "{}",
                                            UndeterminedSpringAnchoringKind::FundamentalOrOvertone
                                        ),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::LowestKey),
                                        format!("{}", UndeterminedSpringAnchoringKind::LowestKey),
                                    )
                                    .changed();
                                change_anchoring |= ui
                                    .selectable_value(
                                        fallback,
                                        Box::new(UndeterminedSpringAnchoringKind::HighestKey),
                                        format!("{}", UndeterminedSpringAnchoringKind::HighestKey),
                                    )
                                    .changed();
                            });
                        }
                    });

                    if change_anchoring {
                        adaptor.send(FromUi::ToStrategy(ToStrategy::TwoStep(
                            ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                                ToStaticNeighbourhoodsAsMelody::Reanchor {
                                    time: Instant::now(),
                                },
                            )),
                        )));
                    }

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
                            ui.add(egui::DragValue::new(group_ms).range(0..=1000))
                                .on_hover_text_at_pointer(
                                    "This is to accommodate for the fact that we \
                            don't press and release all keys at exactly the same time.",
                                );
                            ui.label("ms.");
                        });
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
        Self {
            most_common_notes_fallback: Box::new(UndeterminedSpringAnchoringKind::Fundamental),
            most_common_pitch_classes_fallback: Box::new(
                UndeterminedSpringAnchoringKind::Fundamental,
            ),
            min_number_of_common_notes: 2,
            min_number_of_common_pitch_classes: 2,
        }
    }
}
