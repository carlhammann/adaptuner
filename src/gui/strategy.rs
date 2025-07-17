use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    bindable::Bindable,
    config::{ExtendedStrategyConfig, StrategyKind},
    interval::stacktype::r#trait::{FiveLimitStackType, IntervalBasis, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
    strategy::r#trait::StrategyAction,
};

use super::{
    common::show_hide_button,
    editor::{
        binding::{bindable_selector, strategy_action_selector},
        neighbourhood::NeighbourhoodEditor,
        reference::{ReferenceEditor, ReferenceEditorConfig},
        tuning::{TuningEditor, TuningEditorConfig},
    },
    r#trait::GuiShow,
};

pub struct StrategyWindows<T: StackType + 'static> {
    curr_strategy_index: Option<usize>,
    strategies: Vec<ExtendedStrategyConfig<T>>,

    marked_for_deletion: Option<usize>,

    show_new_strategy_window: bool,
    bring_new_strategy_window_to_top: bool,
    templates: &'static [ExtendedStrategyConfig<T>],
    clone_index: Option<usize>,
    template_index: Option<usize>,
    new_strategy_name: String,
    new_strategy_description: String,

    show_tuning_editor: bool,
    tuning_editor: TuningEditor<T>,

    show_reference_editor: bool,
    reference_editor: ReferenceEditor<T>,

    show_neighbourhood_editor: bool,
    neigbourhood_editor: NeighbourhoodEditor<T>,

    show_binding_editor: bool,
    tmp_strategy_action: Option<StrategyAction>,
    tmp_bindable: Bindable,
    tmp_key_name: String,
    tmp_key_name_invalid: bool,
}

impl<T: StackType> StrategyWindows<T> {
    pub fn new(
        strategies: Vec<ExtendedStrategyConfig<T>>,
        templates: &'static [ExtendedStrategyConfig<T>],
        tuning_editor: TuningEditorConfig,
        reference_editor: ReferenceEditorConfig,
    ) -> Self {
        Self {
            curr_strategy_index: None {},
            strategies,
            marked_for_deletion: None {},
            templates,
            show_new_strategy_window: false,
            bring_new_strategy_window_to_top: false,
            clone_index: None {},
            template_index: None {},
            new_strategy_name: String::with_capacity(32),
            new_strategy_description: String::with_capacity(128),
            show_tuning_editor: false,
            show_reference_editor: false,
            show_neighbourhood_editor: false,
            tuning_editor: TuningEditor::new(tuning_editor),
            reference_editor: ReferenceEditor::new(reference_editor),
            neigbourhood_editor: NeighbourhoodEditor::new(),
            show_binding_editor: false,
            tmp_strategy_action: None {},
            tmp_bindable: Bindable::SostenutoPedalDown,
            tmp_key_name: "Space".into(),
            tmp_key_name_invalid: false,
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for StrategyWindows<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SwitchToStrategy { index } => {
                self.curr_strategy_index = Some(*index);
            }
            ToUi::DeleteStrategy { index } => {
                if let Some(curr_strategy_index) = &mut self.curr_strategy_index {
                    // the next two lines must follow exatcly the same logic as [crate::process::FromStrategy]
                    self.strategies.remove(*index);
                    *curr_strategy_index = (*curr_strategy_index).min(self.strategies.len() - 1);
                }
            }
            _ => {}
        }
        self.reference_editor.handle_msg_ref(msg, forward);
        self.tuning_editor.handle_msg_ref(msg, forward);
        self.neigbourhood_editor.handle_msg_ref(msg, forward);
    }
}

fn strategy_picker<T: IntervalBasis>(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    index: &mut Option<usize>,
    strategies: &[ExtendedStrategyConfig<T>],
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(index.map_or("", |i| &strategies[i].name))
        .show_ui(ui, |ui| {
            for (i, esc) in strategies.iter().enumerate() {
                let r = ui.selectable_value(index, Some(i), &esc.name);
                if r.clicked() {
                    changed = true;
                }
                if !esc.description.is_empty() {
                    r.on_hover_text_at_pointer(&esc.description);
                }
            }
        });
    changed
}

pub struct AsStrategyPicker<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);

impl<'a, T: StackType> GuiShow<T> for AsStrategyPicker<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsStrategyPicker(x) = self;
        let close_popup = |ui: &mut egui::Ui| ui.memory_mut(|m| m.close_popup());
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("top level strategy picker")
                .selected_text(x.curr_strategy_index.map_or("", |i| &x.strategies[i].name))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    egui::Grid::new("strategy picker grid").show(ui, |ui| {
                        for (i, esc) in x.strategies.iter().enumerate() {
                            let r =
                                ui.selectable_value(&mut x.curr_strategy_index, Some(i), &esc.name);
                            if r.changed() {
                                let _ = forward.send(FromUi::SwitchToStrategy {
                                    index: i,
                                    time: Instant::now(),
                                });
                            }
                            if r.clicked() {
                                x.marked_for_deletion = None {};
                                close_popup(ui);
                            }
                            if !esc.description.is_empty() {
                                r.on_hover_text_at_pointer(&esc.description);
                            }

                            let r = ui.add_enabled(
                                x.strategies.len() > 1,
                                egui::Button::new(if x.marked_for_deletion == Some(i) {
                                    "really delete"
                                } else {
                                    "delete"
                                }),
                            );
                            if r.clicked() {
                                if x.marked_for_deletion == Some(i) {
                                    let _ = forward.send(FromUi::DeleteStrategy {
                                        index: i,
                                        time: Instant::now(),
                                    });
                                    x.marked_for_deletion = None {};
                                    close_popup(ui);
                                } else {
                                    x.marked_for_deletion = Some(i);
                                }
                            }

                            ui.end_row();
                        }
                    });

                    ui.separator();
                    if ui.button("new strategy").clicked() {
                        if x.show_new_strategy_window {
                            x.bring_new_strategy_window_to_top = true;
                        }
                        x.show_new_strategy_window = true;
                        x.marked_for_deletion = None {};
                        close_popup(ui);
                    }
                });

            ui.separator();

            if let Some(ix) = x.curr_strategy_index {
                match x.strategies[ix].strategy_kind() {
                    StrategyKind::StaticTuning => {
                        ui.horizontal(|ui| {
                            show_hide_button(ui, &mut x.show_tuning_editor, "global tuning");
                            show_hide_button(ui, &mut x.show_reference_editor, "reference");
                            show_hide_button(
                                ui,
                                &mut x.show_neighbourhood_editor,
                                "neighbourhoods",
                            );
                            show_hide_button(ui, &mut x.show_binding_editor, "bindings");
                        });
                    }
                }
            }
        });
    }
}

pub struct AsWindows<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);

impl<'a, T: FiveLimitStackType + PartialEq> GuiShow<T> for AsWindows<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        self.display_new_strategy_window(ui, forward);
        self.display_binding_window(ui, forward);

        let AsWindows(x) = self;
        let ctx = ui.ctx();

        if x.curr_strategy_index.is_none() {
            return;
        }
        let curr_strategy_index = x.curr_strategy_index.unwrap();

        let current_name = &x.strategies[curr_strategy_index].name;

        if x.show_tuning_editor {
            egui::containers::Window::new(format!("global tuning ({current_name})"))
                .id(egui::Id::new("global tuning window"))
                .open(&mut x.show_tuning_editor)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    x.tuning_editor.show(ui, forward);
                });
        }

        if x.show_reference_editor {
            egui::containers::Window::new(format!("reference ({current_name})"))
                .id(egui::Id::new("reference window"))
                .open(&mut x.show_reference_editor)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    x.reference_editor.show(ui, forward);
                });
        }

        if x.show_neighbourhood_editor {
            egui::containers::Window::new(format!("neighbourhoods ({current_name})"))
                .id(egui::Id::new("neighbourhoods window"))
                .open(&mut x.show_neighbourhood_editor)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    x.neigbourhood_editor.show(ui, forward);
                });
        }
    }
}

impl<'a, T: StackType> AsWindows<'a, T> {
    fn display_new_strategy_window(
        &mut self,
        ui: &mut egui::Ui,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let AsWindows(x) = self;
        let ctx = ui.ctx();
        let id = egui::Id::new("new strategy");
        if x.bring_new_strategy_window_to_top {
            let layer_id = egui::LayerId::new(egui::Order::Middle, id);
            ctx.move_to_top(layer_id);
            x.bring_new_strategy_window_to_top = false;
        }

        if x.show_new_strategy_window {
            egui::containers::Window::new("new strategy")
                .id(id)
                .collapsible(false)
                .resizable(false)
                .open(&mut x.show_new_strategy_window)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("copy of");

                        strategy_picker(
                            ui,
                            "clone strategy picker",
                            &mut x.clone_index,
                            &x.strategies,
                        );
                        if x.clone_index.is_some() {
                            x.template_index = None {};
                        }

                        ui.separator();

                        ui.label("from template");

                        strategy_picker(
                            ui,
                            "template strategy picker",
                            &mut x.template_index,
                            x.templates,
                        );
                        if x.template_index.is_some() {
                            x.clone_index = None {};
                        }
                    });

                    ui.separator();
                    ui.label("name:");
                    ui.text_edit_singleline(&mut x.new_strategy_name);

                    ui.separator();
                    ui.label("optional description:");
                    ui.text_edit_multiline(&mut x.new_strategy_description);

                    ui.separator();

                    let finished = !x.new_strategy_name.is_empty()
                        & (x.template_index.is_some() | x.clone_index.is_some());

                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(finished, egui::Button::new("create"))
                            .clicked()
                        {
                            x.clone_index.map(|i| {
                                let new_strategy = ExtendedStrategyConfig {
                                    name: x.new_strategy_name.clone(),
                                    description: x.new_strategy_description.clone(),
                                    config: x.strategies[i].config.clone(),
                                    bindings: x.strategies[i].bindings.clone(),
                                };
                                x.strategies.push(new_strategy);
                                let _ = forward.send(FromUi::CloneStrategy {
                                    index: i,
                                    time: Instant::now(),
                                });
                            });

                            x.template_index.map(|i| {
                                let new_strategy = ExtendedStrategyConfig {
                                    name: x.new_strategy_name.clone(),
                                    description: x.new_strategy_description.clone(),
                                    config: x.templates[i].config.clone(),
                                    bindings: x.strategies[i].bindings.clone(),
                                };
                                x.strategies.push(new_strategy);
                                let _ = forward.send(FromUi::AddStrategyFromTemplate {
                                    index: i,
                                    time: Instant::now(),
                                });
                            });

                            x.new_strategy_name.clear();
                            x.new_strategy_description.clear();
                            x.clone_index = None {};
                            x.template_index = None {};
                            // x.show_new_strategy_window = false;
                        }
                    });
                });
        }
    }

    fn display_binding_window(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsWindows(x) = self;
        let ctx = ui.ctx();

        if x.curr_strategy_index.is_none() {
            return;
        }
        let curr_strategy_index = x.curr_strategy_index.unwrap();
        let current = &mut x.strategies[curr_strategy_index];

        egui::containers::Window::new(format!("bindings ({})", current.name))
            .id(egui::Id::new("bindings window"))
            .collapsible(false)
            .resizable(false)
            .open(&mut x.show_binding_editor)
            .show(ctx, |ui| {
                egui::Grid::new("active binding grid").show(ui, |ui| {
                    for (k, v) in current.bindings.iter_mut() {
                        ui.label(format!("{k}"));
                        // *x.tmp_strategy_action = v;
                        // if strategy_action_selector(
                        //     ui,
                        //     current.strategy_kind(),
                        //     *k,
                        //    v,
                        //     // &mut x.tmp_strategy_action,
                        // ) {
                        //     if let Some(action) = x.tmp_strategy_action {
                        //         current.bindings.insert(*k, action);
                        //     } else {
                        //         current.bindings.remove(k);
                        //     }
                        // }
                        ui.label(format!("{v}"));
                        ui.end_row();
                    }
                });

                ui.separator();
                ui.label("add or change a binding:");
                ui.horizontal(|ui| {
                    bindable_selector(
                        ui,
                        &mut x.tmp_bindable,
                        &mut x.tmp_key_name,
                        &mut x.tmp_key_name_invalid,
                    );
                    x.tmp_strategy_action = current.bindings.get(&x.tmp_bindable).map(|x| *x);
                    if strategy_action_selector(
                        ui,
                        current.strategy_kind(),
                        x.tmp_bindable,
                        &mut x.tmp_strategy_action,
                    ) {
                        if let Some(action) = x.tmp_strategy_action {
                            current.bindings.insert(x.tmp_bindable, action);
                        } else {
                            current.bindings.remove(&x.tmp_bindable);
                        }
                        let _ = forward.send(FromUi::BindAction {
                            action: x.tmp_strategy_action,
                            bindable: x.tmp_bindable,
                        });
                    }
                });
            });
    }
}
