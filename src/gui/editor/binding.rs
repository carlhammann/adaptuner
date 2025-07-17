use eframe::egui;

use crate::{bindable::Bindable, config::StrategyKind, strategy::r#trait::StrategyAction};

pub fn bindable_selector(
    ui: &mut egui::Ui,
    tmp_bindable: &mut Bindable,
    tmp_key_name: &mut String,
    tmp_key_name_invalid: &mut bool,
) {
    egui::ComboBox::from_id_salt("bindable selector")
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(format!("{tmp_bindable}"))
        .show_ui(ui, |ui| {
            let close_popup = |ui: &mut egui::Ui| ui.memory_mut(|m| m.close_popup());

            for (bindable, description) in [
                (Bindable::SostenutoPedalDown, "If this is set, the sostenuto pedal will lose its normal function."),
                (Bindable::SostenutoPedalUp, "If this is set, the sostenuto pedal will lose its normal function."),
                (Bindable::SoftPedalDown, "If this is set, the soft pedal will lose its normal function."),
                (Bindable::SoftPedalUp, "If this is set, the soft pedal will lose its normal function."),
            ] {
                let r = ui
                    .selectable_value(tmp_bindable, bindable, format!("{bindable}"))
                    .on_hover_text_at_pointer(description);

                if r.clicked() {
                    close_popup(ui);
                }
            }

            ui.horizontal(|ui| {
                ui.style_mut().spacing.text_edit_width = 3.0 * ui.style().spacing.interact_size.y;
                if let Bindable::KeyDown(key) = tmp_bindable {
                    let mut b = true;
                    ui.selectable_value(&mut b, true, "key press on");
                    let r = ui
                        .text_edit_singleline(tmp_key_name)
                        .on_hover_text_at_pointer(
                            r#"Key name or single character: 
• A, B, ...
• 1, 2, ...
• F1, F2, ...
• Esc, Backspace, ...
• Some keys have several names like 'Minus' and '-'"#,
                        );
                    if r.gained_focus() {
                        tmp_key_name.clear();
                        *tmp_key_name_invalid = true;
                    }
                    if r.changed() {
                        if let Some(new_key) = egui::Key::from_name(tmp_key_name) {
                            *tmp_key_name_invalid = false;
                            *key = new_key;
                        } else {
                            *tmp_key_name_invalid = true;
                        }
                    }
                    if r.lost_focus() && !*tmp_key_name_invalid {
                        close_popup(ui);
                    }
                    if *tmp_key_name_invalid {
                        ui.label(
                            egui::RichText::new("invalid key")
                                .color(ui.style().visuals.warn_fg_color),
                        );
                    }
                } else {
                    ui.selectable_value(
                        tmp_bindable,
                        Bindable::KeyDown(egui::Key::Space),
                        "key press on",
                    );
                    *tmp_key_name = "Space".into();
                    *tmp_key_name_invalid = false;
                    ui.add_enabled(false, egui::TextEdit::singleline(tmp_key_name));
                }
            });
        });
}

pub fn strategy_action_selector(
    ui: &mut egui::Ui,
    strategy_kind: StrategyKind,
    bindable: Bindable,
    tmp_strategy_action: &mut Option<StrategyAction>,
) -> bool {
    let mut changed = false;

    egui::ComboBox::from_id_salt(bindable)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(tmp_strategy_action.map_or("no action".into(), |action| format!("{action}")))
        .show_ui(ui, |ui| {
            let close_popup = |ui: &mut egui::Ui| {
                ui.memory_mut(|m| m.close_popup());
            };

            let r = ui.selectable_value(tmp_strategy_action, None {}, "no action");
            if r.clicked() {
                changed = r.changed();
                close_popup(ui);
            }

            if strategy_kind.increment_neighbourhood_index_allowed() {
                ui.horizontal(|ui| {
                    if let Some(StrategyAction::IncrementNeighbourhoodIndex(i)) =
                        tmp_strategy_action
                    {
                        let mut b = true;
                        ui.selectable_value(&mut b, true, "increment neighbourhood index by");
                        let r = ui.add(egui::DragValue::new(i));
                        if r.changed() {
                            changed = true;
                        }
                        if r.lost_focus() || r.drag_stopped() {
                            close_popup(ui);
                        }
                    } else {
                        if ui
                            .selectable_value(
                                tmp_strategy_action,
                                Some(StrategyAction::IncrementNeighbourhoodIndex(1)),
                                "increment neighbourhood index by",
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        let mut i = 1;
                        ui.add_enabled(false, egui::DragValue::new(&mut i));
                    }
                });
            }

            if strategy_kind.set_reference_to_lowest_allowed() {
                let r = ui.selectable_value(
                    tmp_strategy_action,
                    Some(StrategyAction::SetReferenceToLowest),
                    "set reference to lowest sounding note",
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if strategy_kind.set_reference_to_highest_allowed() {
                let r = ui.selectable_value(
                    tmp_strategy_action,
                    Some(StrategyAction::SetReferenceToHighest),
                    "set reference to highest sounding note",
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }
        });

    changed
}
