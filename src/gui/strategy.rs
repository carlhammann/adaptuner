use std::{sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    bindable::Bindable,
    config::{ExtendedStrategyConfig, StrategyKind},
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
    strategy::r#trait::StrategyAction,
    util::list_action::ListAction,
};

use super::{
    common::{ListEdit, ListEditOpts, ListPicker, SmallFloatingWindow},
    editor::{
        binding::{bindable_selector, strategy_action_selector},
        neighbourhood::NeighbourhoodEditor,
        reference::{ReferenceEditor, ReferenceEditorConfig},
        tuning::{TuningEditor, TuningEditorConfig},
    },
    r#trait::GuiShow,
};

pub struct StrategyWindows<T: StackType + 'static> {
    strategy_list_editor_window: SmallFloatingWindow,
    strategies: ListEdit<ExtendedStrategyConfig<T>>,
    templates: ListPicker<'static, ExtendedStrategyConfig<T>>,

    tuning_editor_window: SmallFloatingWindow,
    tuning_editor: TuningEditor<T>,

    reference_editor_window: SmallFloatingWindow,
    reference_editor: ReferenceEditor<T>,

    neighbourhood_editor_window: SmallFloatingWindow,
    neighbourhood_editor: NeighbourhoodEditor<T>,

    binding_editor_window: SmallFloatingWindow,
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
            strategies: ListEdit::new(strategies),
            templates: ListPicker::new(templates),
            strategy_list_editor_window: SmallFloatingWindow::new(egui::Id::new(
                "strategy_list_editor_window",
            )),
            tuning_editor_window: SmallFloatingWindow::new(egui::Id::new("tuning_editor_window")),
            tuning_editor: TuningEditor::new(tuning_editor),
            reference_editor_window: SmallFloatingWindow::new(egui::Id::new(
                "reference_editor_window",
            )),
            reference_editor: ReferenceEditor::new(reference_editor),
            neighbourhood_editor_window: SmallFloatingWindow::new(egui::Id::new(
                "neigbourhood_editor_window",
            )),
            neighbourhood_editor: NeighbourhoodEditor::new(),
            binding_editor_window: SmallFloatingWindow::new(egui::Id::new("binding_editor_window")),
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
            ToUi::CurrentStrategyIndex(index) => {
                if let Some(i) = index {
                    self.strategies.apply(ListAction::Select(*i));
                } else {
                    self.strategies.apply(ListAction::Deselect);
                }
            }
            _ => {}
        }
        self.reference_editor.handle_msg_ref(msg, forward);
        self.tuning_editor.handle_msg_ref(msg, forward);
        self.neighbourhood_editor.handle_msg_ref(msg, forward);
    }
}

pub struct AsStrategyPicker<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);

impl<'a, T: StackType> GuiShow<T> for AsStrategyPicker<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsStrategyPicker(x) = self;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("strategy picker")
                .selected_text(x.strategies.current_selected().map_or("", |x| &x.name))
                .show_ui(ui, |ui| {
                    ui.shrink_width_to_current();
                    if let Some((i, _esc)) =
                        x.strategies
                            .show_as_list_picker(ui, |x| &x.name, |x| Some(&x.description))
                    {
                        let _ = forward.send(FromUi::StrategyListAction {
                            action: ListAction::Select(i),
                            time: Instant::now(),
                        });
                    }

                    ui.separator();

                    x.strategy_list_editor_window
                        .show_hide_button(ui, "edit strategies");
                });

            ui.separator();

            if let Some(esc) = x.strategies.current_selected() {
                match esc.strategy_kind() {
                    StrategyKind::StaticTuning => {
                        ui.horizontal(|ui| {
                            x.tuning_editor_window.show_hide_button(ui, "global tuning");
                            x.reference_editor_window.show_hide_button(ui, "reference");
                            x.neighbourhood_editor_window
                                .show_hide_button(ui, "neighbourhoods");
                            x.binding_editor_window.show_hide_button(ui, "bindings");
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
        self.display_strategy_list_editor_window(ui, forward);
        self.display_binding_window(ui, forward);

        let AsWindows(x) = self;
        let ctx = ui.ctx();

        if let Some(ExtendedStrategyConfig {
            name: current_name, ..
        }) = x.strategies.current_selected()
        {
            x.tuning_editor_window
                .show(&format!("global tuning ({current_name})"), ctx, |ui| {
                    x.tuning_editor.show(ui, forward);
                });

            x.reference_editor_window
                .show(&format!("reference ({current_name})"), ctx, |ui| {
                    x.reference_editor.show(ui, forward);
                });

            x.neighbourhood_editor_window.show(
                &format!("neighbourhoods ({current_name})"),
                ctx,
                |ui| {
                    x.neighbourhood_editor.show(ui, forward);
                },
            );
        }
    }
}

impl<'a, T: StackType> AsWindows<'a, T> {
    fn display_strategy_list_editor_window(
        &mut self,
        ui: &mut egui::Ui,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let AsWindows(x) = self;
        let ctx = ui.ctx();
        x.strategy_list_editor_window
            .show("edit strategies", ctx, |ui| {
                ui.shrink_width_to_current();
                let action = x.strategies.show_with_add(
                    ui,
                    "strategy editor",
                    &ListEditOpts {
                        empty_allowed: false,
                        select_allowed: true,
                        no_selection_allowed: false,
                        delete_allowed: true,
                    },
                    |ui, elem| {
                        ui.add(egui::TextEdit::singleline(&mut elem.name).min_size(vec2(
                            ui.style().spacing.text_edit_width / 2.0,
                            ui.style().spacing.interact_size.y,
                        )));
                        ui.add(
                            egui::TextEdit::multiline(&mut elem.description)
                                .min_size(vec2(
                                    ui.style().spacing.text_edit_width,
                                    ui.style().spacing.interact_size.y,
                                ))
                                .desired_rows(1),
                        );
                    },
                    |ui, elems, selected| {
                        let mut new: Option<ExtendedStrategyConfig<T>> = None {};
                        ui.horizontal(|ui| {
                            ui.label("add new");
                            if ui.button("copy of currently selected").clicked() {
                                new = selected.map(|i| elems[i].clone());
                                x.templates.deselect();
                            }
                            ui.separator();
                            ui.label("from template");

                            egui::ComboBox::from_id_salt("template picker")
                                .selected_text(
                                    x.templates.current_selected().map_or("", |x| &x.name),
                                )
                                .show_ui(ui, |ui| {
                                    ui.shrink_width_to_current();
                                    if let Some((_, template)) =
                                        x.templates.show(ui, |x| &x.name, |x| Some(&x.description))
                                    {
                                        new = Some(template.clone());
                                        x.templates.deselect();
                                    }
                                });
                        });

                        new
                    },
                );

                if let Some(action) = action {
                    let _ = forward.send(FromUi::StrategyListAction {
                        action,
                        time: Instant::now(),
                    });
                }
            });
    }

    fn display_binding_window(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsWindows(x) = self;

        if let Some(current) = x.strategies.current_selected_mut() {
            let ctx = ui.ctx();

            x.binding_editor_window
                .show(&format!("bindings ({})", current.name), ctx, |ui| {
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
}
