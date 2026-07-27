use eframe::egui::{self, Popup};

use crate::{
    bindable::{BindableEvent, BindableStrategyAction},
    config::StrategyConfig,
    gui::r#trait::{GuiShow, UiAdaptor},
    interval::stacktype::r#trait::StackType,
    util::ordered_locks::{OrderedLocks, Zero},
};

pub struct BindingEditor {
    tmp_event: BindableEvent,
    tmp_key_name: String,
    tmp_key_name_invalid: bool,
    tmp_action: Option<BindableStrategyAction>,
    changed_binding: Option<(BindableEvent, Option<BindableStrategyAction>)>,
}

impl BindingEditor {
    pub fn new() -> Self {
        Self {
            tmp_event: BindableEvent::SostenutoPedalDown,
            tmp_key_name: String::with_capacity(16),
            tmp_key_name_invalid: true,
            tmp_action: None {},
            changed_binding: None {},
        }
    }
}

impl<T: StackType> GuiShow<T> for BindingEditor {
    fn show<A: UiAdaptor<StackType = T>>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero> {
        (_, adaptor) = adaptor.active_strategy_mut(|strat, _| {
            ui.collapsing("key bindings", |ui| {
                ui.vertical(|ui| {
                    ui.shrink_width_to_current();
                    egui::Grid::new("binding_editor_grid").show(ui, |ui| {
                        for (k, v) in strat.bindings().iter() {
                            ui.label(format!("{k}"));

                            self.tmp_action = Some(*v);
                            if strategy_action_selector(ui, strat, *k, &mut self.tmp_action) {
                                if self.changed_binding.is_none() {
                                    self.changed_binding = Some((*k, self.tmp_action));
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

                    self.tmp_action = None {};

                    ui.add(
                        egui::Label::new("add a binding:").wrap_mode(egui::TextWrapMode::Extend),
                    );
                    ui.horizontal(|ui| {
                        bindable_selector(
                            ui,
                            &mut self.tmp_event,
                            &mut self.tmp_key_name,
                            &mut self.tmp_key_name_invalid,
                        );
                        self.tmp_action = strat.bindings().get(&self.tmp_event).map(|x| *x);
                        if strategy_action_selector(ui, strat, self.tmp_event, &mut self.tmp_action)
                        {
                            if self.changed_binding.is_none() {
                                self.changed_binding = Some((self.tmp_event, self.tmp_action));
                            }
                        }

                        if let Some((bindable, action)) = self.changed_binding {
                            if let Some(action) = action {
                                strat.bindings_mut().insert(bindable, action);
                            } else {
                                strat.bindings_mut().remove(&bindable);
                            }

                            self.changed_binding = None {}
                        }
                    });
                });
            });
        });

        adaptor
    }
}

fn bindable_selector(
    ui: &mut egui::Ui,
    tmp_event: &mut BindableEvent,
    tmp_key_name: &mut String,
    tmp_key_name_invalid: &mut bool,
) {
    egui::ComboBox::from_id_salt("bindable selector")
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(format!("{tmp_event}"))
        .show_ui(ui, |ui| {
            let popup_id = ui.id();
            let close_popup = |ui: &mut egui::Ui| Popup::close_id(ui.ctx(), popup_id);

            for (bindable, description) in [
                (
                    BindableEvent::SostenutoPedalDown,
                    "If this is set, the sostenuto pedal will lose its normal function.",
                ),
                (
                    BindableEvent::SostenutoPedalUp,
                    "If this is set, the sostenuto pedal will lose its normal function.",
                ),
                (
                    BindableEvent::SoftPedalDown,
                    "If this is set, the soft pedal will lose its normal function.",
                ),
                (
                    BindableEvent::SoftPedalUp,
                    "If this is set, the soft pedal will lose its normal function.",
                ),
            ] {
                let r = ui
                    .selectable_value(tmp_event, bindable, format!("{bindable}"))
                    .on_hover_text_at_pointer(description);

                if r.clicked() {
                    close_popup(ui);
                }
            }

            ui.horizontal(|ui| {
                ui.style_mut().spacing.text_edit_width = 3.0 * ui.style().spacing.interact_size.y;
                if let BindableEvent::KeyPress(key) = tmp_event {
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
                        tmp_event,
                        BindableEvent::KeyPress(egui::Key::Space),
                        "key press on",
                    );
                    *tmp_key_name = "Space".into();
                    *tmp_key_name_invalid = false;
                    ui.add_enabled_ui(false, |ui| ui.text_edit_singleline(tmp_key_name));
                }
            });
        });
}

fn strategy_action_selector<T: StackType>(
    ui: &mut egui::Ui,
    active_strategy: &StrategyConfig<T>,
    event: BindableEvent,
    tmp_action: &mut Option<BindableStrategyAction>,
) -> bool {
    let mut changed = false;

    egui::ComboBox::from_id_salt(event)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(tmp_action.map_or("".into(), |action| format!("{action}")))
        .show_ui(ui, |ui| {
            let popup_id = ui.id();
            let close_popup = |ui: &mut egui::Ui| Popup::close_id(ui.ctx(), popup_id);

            if active_strategy
                .reacts_to_bound(BindableStrategyAction::IncrementNeighbourhoodIndex(0))
            {
                ui.horizontal(|ui| {
                    if let Some(BindableStrategyAction::IncrementNeighbourhoodIndex(i)) = tmp_action
                    {
                        let mut b = true;
                        ui.selectable_value(&mut b, true, "skip to scale at offset");
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
                                tmp_action,
                                Some(BindableStrategyAction::IncrementNeighbourhoodIndex(1)),
                                "skip to scale at offset",
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

            if active_strategy.reacts_to_bound(BindableStrategyAction::SetReferenceToLowest) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::SetReferenceToLowest),
                    format!("{}", BindableStrategyAction::SetReferenceToLowest),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if active_strategy.reacts_to_bound(BindableStrategyAction::SetReferenceToHighest) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::SetReferenceToHighest),
                    format!("{}", BindableStrategyAction::SetReferenceToHighest),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if active_strategy.reacts_to_bound(BindableStrategyAction::SetReferenceToCurrent) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::SetReferenceToCurrent),
                    format!("{}", BindableStrategyAction::SetReferenceToCurrent),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if active_strategy.reacts_to_bound(BindableStrategyAction::ToggleChordMatching) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::ToggleChordMatching),
                    format!("{}", BindableStrategyAction::ToggleChordMatching),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if active_strategy.reacts_to_bound(BindableStrategyAction::ToggleReanchor) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::ToggleReanchor),
                    format!("{}", BindableStrategyAction::ToggleReanchor),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }

            if active_strategy.reacts_to_bound(BindableStrategyAction::Reset) {
                let r = ui.selectable_value(
                    tmp_action,
                    Some(BindableStrategyAction::Reset),
                    format!("{}", BindableStrategyAction::Reset),
                );
                if r.clicked() {
                    changed = r.changed();
                    close_popup(ui);
                }
            }
        });

    changed
}
