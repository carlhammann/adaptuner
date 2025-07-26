use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    bindable::{Bindable, Bindings},
    config::{StrategyKind, StrategyNames},
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
    strategy::r#trait::StrategyAction,
    util::list_action::ListAction,
};

use super::{
    common::{
        CorrectionSystemChooser, ListEdit, ListEditOpts, OwningListEdit, SmallFloatingWindow,
    },
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
    strategies: OwningListEdit<(StrategyNames, Bindings)>,

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
    pub fn strategies(&self) -> &[(StrategyNames, Bindings)] {
        self.strategies.elems()
    }

    pub fn new(
        strategies: Vec<(StrategyNames, Bindings)>,
        tuning_editor: TuningEditorConfig,
        reference_editor: ReferenceEditorConfig,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    ) -> Self {
        Self {
            strategies: OwningListEdit::new(strategies),
            strategy_list_editor_window: SmallFloatingWindow::new(egui::Id::new(
                "strategy_list_editor_window",
            )),
            tuning_editor_window: SmallFloatingWindow::new(egui::Id::new("tuning_editor_window")),
            tuning_editor: TuningEditor::new(tuning_editor, correction_system_chooser.clone()),
            reference_editor_window: SmallFloatingWindow::new(egui::Id::new(
                "reference_editor_window",
            )),
            reference_editor: ReferenceEditor::new(reference_editor, correction_system_chooser),
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
                // if let Some((strn, bnd)) = self.strategies.current_selected_mut() {
                //     strn.neighbourhood_names = self.neighbourhood_editor.get_all().into();
                // }
                if let Some(i) = index {
                    self.strategies.apply(ListAction::Select(*i));
                } else {
                    self.strategies.apply(ListAction::Deselect);
                }
                // if let Some((strn, _)) = self.strategies.current_selected() {
                //     self.neighbourhood_editor.set_all(&strn.neighbourhood_names);
                // }
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
                .selected_text(x.strategies.current_selected().map_or("", |x| &x.0.name))
                .show_ui(ui, |ui| {
                    ui.shrink_width_to_current();
                    if let Some((i, _)) = x.strategies.show_as_list_picker(
                        ui,
                        |x| &x.0.name,
                        |x| Some(&x.0.description),
                    ) {
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

            if let Some(strn) = x.strategies.current_selected() {
                match strn.0.strategy_kind {
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

        if let Some((strn, _)) = x.strategies.current_selected_mut() {
            let current_name = &strn.name;
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
                    x.neighbourhood_editor
                        .show(ui, &mut strn.neighbourhood_names, forward);
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
                let list_edit_res = x.strategies.show(
                    ui,
                    "strategy editor",
                    ListEditOpts {
                        empty_allowed: false,
                        select_allowed: true,
                        no_selection_allowed: false,
                        delete_allowed: true,
                        show_one: Box::new(|ui, _i, elem| {
                            ui.add(egui::TextEdit::singleline(&mut elem.0.name).min_size(vec2(
                                ui.style().spacing.text_edit_width / 2.0,
                                ui.style().spacing.interact_size.y,
                            )));
                            ui.add(
                                egui::TextEdit::multiline(&mut elem.0.description)
                                    .min_size(vec2(
                                        ui.style().spacing.text_edit_width,
                                        ui.style().spacing.interact_size.y,
                                    ))
                                    .desired_rows(1),
                            );
                            None::<()> {}
                        }),
                        clone: Some(Box::new(|ui, _elems, selected| {
                            if let Some(i) = selected {
                                if ui.button("create copy of selected").clicked() {
                                    Some(i)
                                } else {
                                    None {}
                                }
                            } else {
                                None {}
                            }
                        })),
                    },
                );

                match list_edit_res {
                    super::common::ListEditResult::Message(_) => unreachable!(),
                    super::common::ListEditResult::Action(action) => {
                        let _ = forward.send(FromUi::StrategyListAction {
                            action,
                            time: Instant::now(),
                        });
                    }
                    super::common::ListEditResult::None => {}
                }
            });
    }

    fn display_binding_window(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsWindows(x) = self;

        if let Some((strn, bindings)) = x.strategies.current_selected_mut() {
            let ctx = ui.ctx();

            x.binding_editor_window
                .show(&format!("bindings ({})", strn.name), ctx, |ui| {
                    egui::Grid::new("active binding grid").show(ui, |ui| {
                        for (k, v) in bindings.iter_mut() {
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
                        x.tmp_strategy_action = bindings.get(&x.tmp_bindable).map(|x| *x);
                        if strategy_action_selector(
                            ui,
                            strn.strategy_kind,
                            x.tmp_bindable,
                            &mut x.tmp_strategy_action,
                        ) {
                            if let Some(action) = x.tmp_strategy_action {
                                bindings.insert(x.tmp_bindable, action);
                            } else {
                                bindings.remove(&x.tmp_bindable);
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
