use std::time::Instant;

use eframe::egui;

use crate::{
    config::{HarmonyStrategyConfig, StrategyConfig},
    gui::{
        common::{note_picker, rational_drag_value},
        r#trait::{GuiShow, UiAdaptor},
    },
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::{FromUi, ToHarmony, ToHarmonySprings, ToStrategy, ToTwoStep},
    notename::{correction::Correction, HasNoteNames, NoteNameStyle},
    strategy::harmony::springs::{
        HarmonySpringsConfig, HarmonySpringsProvider, RodOrSprings, Spring,
    },
    util::ordered_locks::{Nat, Zero},
};

pub struct HarmonySpringsEditor<T: StackType> {
    provider_base_note: Stack<T>,
    tmp_temperaments: Vec<bool>,
    tmp_correction: Correction<T>,
    tmp_stack: Stack<T>,
}

impl<T: StackType> HarmonySpringsEditor<T> {
    pub fn new() -> Self {
        Self {
            provider_base_note: Stack::new_zero(),
            tmp_temperaments: vec![false; T::num_temperaments()],
            tmp_correction: Correction::new_zero(),
            tmp_stack: Stack::new_zero(),
        }
    }
}

impl<T: StackType + HasNoteNames> HarmonySpringsEditor<T> {
    fn show_harmony_springs_config<L: Nat>(
        &mut self,
        conf: &mut HarmonySpringsConfig<T>,
        ui: &mut egui::Ui,
        adaptor: &UiAdaptor<T, L>,
    ) {
        let HarmonySpringsConfig {
            enable,
            memo_springs,
            min_keys,
            lower_intervals_are_more_stable,
            provider,
        } = conf;

        let send = |msg: ToHarmonySprings| {
            adaptor.send(FromUi::ToStrategy(ToStrategy::TwoStep(
                ToTwoStep::ToHarmonyStrategy(ToHarmony::Springs(msg)),
            )));
        };
        ui.collapsing("harmony springs", |ui| {
            ui.vertical_centered(|ui| {
                if ui
                    .button(if *enable { "disable" } else { "enable" })
                    .clicked()
                {
                    *enable = !*enable;
                    send(ToHarmonySprings::Recalculate {
                        time: Instant::now(),
                    });
                }
            });

            ui.vertical(|ui| {
                if !*enable {
                    ui.disable();
                }

                ui.horizontal(|ui| {
                    ui.label("Only tune if there are at least");
                    if ui
                        .add(egui::DragValue::new(min_keys).range(2..=128))
                        .changed()
                    {
                        send(ToHarmonySprings::Recalculate {
                            time: Instant::now(),
                        });
                    }
                    ui.label("sounding notes.");
                });

                ui.horizontal(|ui| {
                    ui.label("Try alternative spring lengths between");
                    let r = ui.button(if *lower_intervals_are_more_stable {
                        "low"
                    } else {
                        "high"
                    });
                    if r.clicked() {
                        *lower_intervals_are_more_stable = !*lower_intervals_are_more_stable;
                        send(ToHarmonySprings::Recalculate {
                            time: Instant::now(),
                        });
                    }
                    ui.label("notes later.");
                    r.on_hover_text_at_pointer(
                        "This is about the order in which options for \
                      springs are tried: If you preserve springs between low notes, \
                      then intervals between higher notes will receive the second, third,... \
                      options for springs before lower intervals do. So, if earlier options \
                      for each spring are \"better in tune\", preserving springs bewteen low \
                      intervals means that lower intervals will be tuned \
                      better that high intervals.",
                    );
                });

                ui.separator();

                show_spring_provider(
                    &mut self.provider_base_note,
                    &mut self.tmp_temperaments,
                    &mut self.tmp_correction,
                    &mut self.tmp_stack,
                    provider,
                    ui,
                    |msg| send(msg),
                );

                ui.separator();

                let r = ui.checkbox(memo_springs, "memoize candidate spring lengths");
                r.on_hover_text_at_pointer(
                    "If checked, adaptuner will use more memory, \
                            but potentially be faster.",
                );
            });
        });
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for HarmonySpringsEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.active_strategy_mut(|mut strat, adaptor| match &mut strat {
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::Springs(conf),
                ..
            } => {
                self.show_harmony_springs_config(conf, ui, &adaptor);
            }
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::List(confs),
                ..
            } => {
                for conf in confs {
                    if let HarmonyStrategyConfig::Springs(conf) = conf {
                        self.show_harmony_springs_config(conf, ui, &adaptor);
                        break;
                    }
                }
            }
            _ => {}
        });
        adaptor
    }
}

/// Todo if you want to use this function for more than one HarmonySpringsProvider, in more than one
/// place in the UI: Make the ids depend on something (also in the functions called by this
/// function)
fn show_spring_provider<T: StackType + HasNoteNames>(
    base_note: &mut Stack<T>,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction<T>,
    tmp_stack: &mut Stack<T>,
    provider: &mut HarmonySpringsProvider<T>,
    ui: &mut egui::Ui,
    send: impl Fn(ToHarmonySprings),
) {
    match provider {
        HarmonySpringsProvider::Mod12 { by_class, octave } => {
            egui::Grid::new("spring_provider_grid")
                .with_row_color(|i, style| {
                    if i % 2 == 0 {
                        Some(style.visuals.faint_bg_color)
                    } else {
                        None {}
                    }
                })
                .show(ui, |ui| {
                    for (i, x) in by_class.iter_mut().enumerate() {
                        if show_rod_or_springs(
                            base_note,
                            tmp_temperaments,
                            tmp_correction,
                            tmp_stack,
                            i,
                            x,
                            ui,
                        ) {
                            send(ToHarmonySprings::ReloadSprings {
                                time: Instant::now(),
                            });
                        }
                        ui.end_row();
                    }
                });
            egui::CollapsingHeader::new(format!("octave: {}", {
                tmp_stack.clone_from(octave);
                tmp_stack.scaled_add(1, &*base_note);
                tmp_stack.corrected_notename(&NoteNameStyle::Full, false)
            }))
            .id_salt("spring_provider_octave")
            .show(ui, |ui| {
                if note_picker(ui, tmp_temperaments, tmp_correction, octave) {
                    send(ToHarmonySprings::ReloadSprings {
                        time: Instant::now(),
                    });
                }
            });
            egui::CollapsingHeader::new(format!(
                "base note used for interval names: {}",
                base_note.corrected_notename(&NoteNameStyle::Full, false)
            ))
            .id_salt("spring_provider_base_note")
            .show(ui, |ui| {
                note_picker(ui, tmp_temperaments, tmp_correction, base_note)
            });
        }
    }
}

/// returns true iff something changed
fn show_rod_or_springs<T: StackType + HasNoteNames>(
    base_note: &Stack<T>,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction<T>,
    tmp_stack: &mut Stack<T>,
    i: usize,
    x: &mut RodOrSprings<T>,
    ui: &mut egui::Ui,
) -> bool {
    let mut changed = false;
    let mut change_to_springs = None {};
    let mut change_to_rod = None {};

    match x {
        RodOrSprings::Rod(length) => {
            ui.vertical_centered(|ui| {
                ui.label(format!("offset {i}: rod"));
                changed = show_length(
                    format!("spring_provider_rod_length{i}"),
                    base_note,
                    tmp_temperaments,
                    tmp_correction,
                    tmp_stack,
                    length,
                    ui,
                );
                if length.key_distance() != i as StackCoeff {
                    ui.label(
                        egui::RichText::new(format!(
                            "This is an interval spanning {} piano keys",
                            length.key_distance(),
                        ))
                        .color(ui.style().visuals.warn_fg_color),
                    );
                }
                if ui.button("change to springs").clicked() {
                    change_to_springs = Some(length.clone());
                    changed = true;
                }
            });
        }

        RodOrSprings::Springs { options } => {
            ui.vertical_centered(|ui| {
                ui.label(format!("offset {i}: springs"));
                let mut delete = None {};
                for (j, spring) in options.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            changed |= show_length(
                                format!("spring_provider_spring_length{i}{j}"),
                                base_note,
                                tmp_temperaments,
                                tmp_correction,
                                tmp_stack,
                                &mut spring.length,
                                ui,
                            );
                            if spring.length.key_distance() != i as StackCoeff {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "This is an interval spanning {} piano keys",
                                        spring.length.key_distance(),
                                    ))
                                    .color(ui.style().visuals.warn_fg_color),
                                );
                            }
                        });
                        ui.label("stiffness:");
                        changed |= rational_drag_value(
                            ui,
                            egui::Id::new(format!("spring_provider_spring_stiffness_{i}{j}")),
                            &mut spring.stiffness,
                        );
                        if ui.button("delete").clicked() {
                            delete = Some(j);
                        }
                    });
                }
                if let Some(j) = delete {
                    options.remove(j);
                    changed = true;
                }

                if ui.button("add option").clicked() {
                    options.push(Spring {
                        length: Stack::new_zero(),
                        stiffness: 1.into(),
                    });
                    changed = true;
                }

                if ui.button("change to rod").clicked() {
                    change_to_rod = Some(options[0].length.clone());
                    changed = true;
                }
            });
        }
    }

    if let Some(length) = change_to_springs {
        *x = RodOrSprings::Springs {
            options: vec![Spring {
                length,
                stiffness: 1.into(),
            }],
        };
    }

    if let Some(length) = change_to_rod {
        *x = RodOrSprings::Rod(length);
    }

    changed
}

/// returns true iff `length` was changed.
fn show_length<T: StackType + HasNoteNames>(
    id_salt: impl std::hash::Hash,
    base_note: &Stack<T>,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction<T>,
    tmp_stack: &mut Stack<T>,
    length: &mut Stack<T>,
    ui: &mut egui::Ui,
) -> bool {
    egui::ComboBox::from_id_salt(id_salt)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(format!("{}", {
            tmp_stack.clone_from(length);
            tmp_stack.scaled_add(1, base_note);
            tmp_stack.corrected_notename(&NoteNameStyle::Full, false)
        }))
        .show_ui(ui, |ui| {
            note_picker(ui, tmp_temperaments, tmp_correction, length)
        })
        .inner;
    false
}
