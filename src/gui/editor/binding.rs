use std::sync::mpsc;

use eframe::egui;

use crate::{
    bindable::{Bindable, Bindings, MidiBindable},
    config::StrategyKind,
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    strategy::r#trait::StrategyAction,
};

pub struct BindingEditor {
    tmp_bindable: Bindable,
    tmp_key_name: String,
    tmp_key_name_invalid: bool,
    tmp_strategy_action: Option<StrategyAction>,
    changed_binding: Option<(Bindable, Option<StrategyAction>)>,
}

impl BindingEditor {
    pub fn new() -> Self {
        Self {
            tmp_bindable: Bindable::Midi(MidiBindable::SostenutoPedalDown),
            tmp_key_name: String::with_capacity(16),
            tmp_key_name_invalid: true,
            tmp_strategy_action: None {},
            changed_binding: None {},
        }
    }

    pub fn show<T: StackType>(
        &mut self,
        ui: &mut egui::Ui,
        strategy_kind: StrategyKind,
        bindings: &mut Bindings<Bindable>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        ui.vertical(|ui| {
            ui.shrink_width_to_current();
            egui::Grid::new("binding_editor_grid").show(ui, |ui| {
                for (k, v) in bindings.iter() {
                    ui.label(format!("{k}"));

                    self.tmp_strategy_action = Some(*v);
                    if strategy_action_selector(
                        ui,
                        strategy_kind,
                        *k,
                        &mut self.tmp_strategy_action,
                    ) {
                        if self.changed_binding.is_none() {
                            self.changed_binding = Some((*k, self.tmp_strategy_action));
                        }
                    }

                    if ui.button("delete").clicked() {
                        if self.changed_binding.is_none() {
                            self.changed_binding = Some((*k, None {}));
                        }
                    }

                    ui.end_row();
                }
            });

            ui.separator();

            self.tmp_strategy_action = None {};

            ui.add(egui::Label::new("add a binding:").wrap_mode(egui::TextWrapMode::Extend));
            ui.horizontal(|ui| {
                bindable_selector(
                    ui,
                    &mut self.tmp_bindable,
                    &mut self.tmp_key_name,
                    &mut self.tmp_key_name_invalid,
                );
                self.tmp_strategy_action = bindings.get(&self.tmp_bindable).map(|x| *x);
                if strategy_action_selector(
                    ui,
                    strategy_kind,
                    self.tmp_bindable,
                    &mut self.tmp_strategy_action,
                ) {
                    if self.changed_binding.is_none() {
                        self.changed_binding = Some((self.tmp_bindable, self.tmp_strategy_action));
                    }
                }

                if let Some((bindable, action)) = self.changed_binding {
                    if let Some(action) = action {
                        bindings.insert(bindable, action);
                    } else {
                        bindings.remove(&bindable);
                    }

                    if let Bindable::Midi(bindable) = bindable {
                        let _ = forward.send(FromUi::BindAction { action, bindable });
                    }

                    self.changed_binding = None {}
                }
            });
        });
    }
}

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
                (
                    Bindable::Midi(MidiBindable::SostenutoPedalDown),
                    "If this is set, the sostenuto pedal will lose its normal function.",
                ),
                (
                    Bindable::Midi(MidiBindable::SostenutoPedalUp),
                    "If this is set, the sostenuto pedal will lose its normal function.",
                ),
                (
                    Bindable::Midi(MidiBindable::SoftPedalDown),
                    "If this is set, the soft pedal will lose its normal function.",
                ),
                (
                    Bindable::Midi(MidiBindable::SoftPedalUp),
                    "If this is set, the soft pedal will lose its normal function.",
                ),
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
                if let Bindable::KeyPress(key) = tmp_bindable {
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
                        Bindable::KeyPress(egui::Key::Space),
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
        .selected_text(tmp_strategy_action.map_or("".into(), |action| format!("{action}")))
        .show_ui(ui, |ui| {
            let close_popup = |ui: &mut egui::Ui| {
                ui.memory_mut(|m| m.close_popup());
            };

            if strategy_kind.action_allowed(&StrategyAction::IncrementNeighbourhoodIndex(0)) {
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

            if strategy_kind.action_allowed(&StrategyAction::SetReferenceToLowest) {
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

            if strategy_kind.action_allowed(&StrategyAction::SetReferenceToHighest) {
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

            if strategy_kind.action_allowed(&StrategyAction::SetReferenceToCurrent) {
                let r = ui.selectable_value(
                    tmp_strategy_action,
                    Some(StrategyAction::SetReferenceToCurrent),
                    "set reference to current chord's reference",
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if strategy_kind.action_allowed(&StrategyAction::ToggleChordMatching) {
                let r = ui.selectable_value(
                    tmp_strategy_action,
                    Some(StrategyAction::ToggleChordMatching),
                    "toggle chord matching",
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if strategy_kind.action_allowed(&StrategyAction::ToggleReanchor) {
                let r = ui.selectable_value(
                    tmp_strategy_action,
                    Some(StrategyAction::ToggleReanchor),
                    "toggle re-setting of the reference on chord match",
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }
        });

    changed
}
