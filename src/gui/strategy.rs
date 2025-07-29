use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    bindable::{Bindable, Bindings},
    config::{ExtractConfig, StrategyKind, StrategyNames},
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::HasNoteNames,
    util::list_action::ListAction,
};

use super::{
    common::{
        CorrectionSystemChooser, ListEdit, ListEditOpts, OwningListEdit, SmallFloatingWindow,
    },
    editor::{
        binding::BindingEditor,
        neighbourhood::NeighbourhoodEditor,
        reference::{ReferenceEditor, ReferenceEditorConfig},
        tuning::{TuningEditor, TuningEditorConfig},
    },
    r#trait::GuiShow,
};

pub struct StrategyWindows<T: StackType + 'static> {
    strategy_list_editor_window: SmallFloatingWindow,
    strategies: OwningListEdit<(StrategyNames, Bindings<Bindable>)>,

    tuning_editor_window: SmallFloatingWindow,
    tuning_editor: TuningEditor<T>,

    reference_editor_window: SmallFloatingWindow,
    reference_editor: ReferenceEditor<T>,

    neighbourhood_editor_window: SmallFloatingWindow,
    neighbourhood_editor: NeighbourhoodEditor<T>,

    binding_editor_window: SmallFloatingWindow,
    binding_editor: BindingEditor,
}

impl<T: StackType> StrategyWindows<T> {
    pub fn strategies(&self) -> &[(StrategyNames, Bindings<Bindable>)] {
        self.strategies.elems()
    }

    pub fn new(
        strategies: Vec<(StrategyNames, Bindings<Bindable>)>,
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
            binding_editor: BindingEditor::new(),
        }
    }

    pub fn restart_from_config(
        &mut self,
        strategies: Vec<(StrategyNames, Bindings<Bindable>)>,
        tuning_editor: TuningEditorConfig,
        reference_editor: ReferenceEditorConfig,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
        _time: Instant,
    ) {
        self.strategies.put_elems(strategies);
        self.tuning_editor = TuningEditor::new(tuning_editor, correction_system_chooser.clone());
        self.reference_editor =
            ReferenceEditor::new(reference_editor, correction_system_chooser.clone());
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
                .selected_text(x.strategies.current_selected().map_or("", |x| &x.0.name))
                .show_ui(ui, |ui| {
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

                    ui.shrink_width_to_current();
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

pub struct AsWindows<'a, T: StackType>(pub &'a mut StrategyWindows<T>);

impl<'a, T: StackType + HasNoteNames + PartialEq> GuiShow<T> for AsWindows<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        self.display_strategy_list_editor_window(ui, forward);

        let AsWindows(x) = self;
        let ctx = ui.ctx();

        if let Some(curr) = x.strategies.current_selected_mut() {
            if ui.ui_contains_pointer() {
                ui.input(|i| {
                    for e in &i.events {
                        match e {
                            egui::Event::Key {
                                key,
                                pressed,
                                repeat,
                                ..
                            } => {
                                if !*pressed || *repeat {
                                    return;
                                }
                                let bindings = &curr.1;
                                if let Some(&action) = bindings.get(&Bindable::KeyPress(*key)) {
                                    let _ = forward.send(FromUi::Action {
                                        action,
                                        time: Instant::now(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }

            let current_name = &curr.0.name;

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
                        .show(ui, &mut curr.0.neighbourhood_names, forward);
                },
            );

            x.binding_editor_window
                .show(&format!("bindings ({current_name})"), ctx, |ui| {
                    x.binding_editor
                        .show(ui, curr.0.strategy_kind, &mut curr.1, forward);
                });
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
}

impl<T: StackType>
    ExtractConfig<(
        Vec<(StrategyNames, Bindings<Bindable>)>,
        TuningEditorConfig,
        ReferenceEditorConfig,
    )> for StrategyWindows<T>
{
    fn extract_config(
        &self,
    ) -> (
        Vec<(StrategyNames, Bindings<Bindable>)>,
        TuningEditorConfig,
        ReferenceEditorConfig,
    ) {
        (
            self.strategies.elems().into(),
            self.tuning_editor.extract_config(),
            self.reference_editor.extract_config(),
        )
    }
}
