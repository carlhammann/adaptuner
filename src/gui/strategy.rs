use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    config::{ExtendedStrategyConfig, StrategyKind},
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
};

use super::{
    common::show_hide_button,
    editor::{
        neighbourhood::NeighbourhoodEditor,
        reference::{ReferenceEditor, ReferenceEditorConfig},
        tuning::{TuningEditor, TuningEditorConfig},
    },
    r#trait::GuiShow,
};

pub struct StrategyWindows<T: StackType + 'static> {
    current_strategy: usize,
    strategies: Vec<(String, StrategyKind)>,

    templates: &'static [ExtendedStrategyConfig<T>],
    show_new_strategy_window: bool,
    clone_index: Option<usize>,
    template_index: Option<usize>,
    new_strategy_name: String,

    show_tuning_editor: bool,
    show_reference_editor: bool,
    show_neighbourhood_editor: bool,
    tuning_editor: TuningEditor<T>,
    reference_editor: ReferenceEditor<T>,
    neigbourhood_editor: NeighbourhoodEditor<T>,
}

impl<T: StackType> StrategyWindows<T> {
    pub fn new(
        strategies: Vec<(String, StrategyKind)>,
        templates: &'static [ExtendedStrategyConfig<T>],
        tuning_editor: TuningEditorConfig,
        reference_editor: ReferenceEditorConfig,
    ) -> Self {
        Self {
            current_strategy: 0,
            strategies,
            templates,
            show_new_strategy_window: false,
            clone_index: None {},
            template_index: None {},
            new_strategy_name: String::with_capacity(64),
            show_tuning_editor: false,
            show_reference_editor: false,
            show_neighbourhood_editor: false,
            tuning_editor: TuningEditor::new(tuning_editor),
            reference_editor: ReferenceEditor::new(reference_editor),
            neigbourhood_editor: NeighbourhoodEditor::new(),
        }
    }

    fn hide_all_windows(&mut self) {
        self.show_new_strategy_window = false;
        self.show_tuning_editor = false;
        self.show_reference_editor = false;
        self.show_neighbourhood_editor = false;
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for StrategyWindows<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SwitchToStrategy { index } => {
                self.current_strategy = *index;
                self.hide_all_windows();
            }
            _ => {}
        }
        self.reference_editor.handle_msg_ref(msg, forward);
        self.tuning_editor.handle_msg_ref(msg, forward);
        self.neigbourhood_editor.handle_msg_ref(msg, forward);
    }
}

pub struct AsStrategyPicker<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);

impl<'a, T: StackType> GuiShow<T> for AsStrategyPicker<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsStrategyPicker(x) = self;
        ui.horizontal(|ui| {
            let (current_name, current_kind) = &x.strategies[x.current_strategy];

            egui::ComboBox::from_id_salt("strategy picker")
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in x.strategies.iter().enumerate() {
                        if ui
                            .selectable_value(&mut x.current_strategy, i, name)
                            .changed()
                        {
                            let _ = forward.send(FromUi::SwitchToStrategy {
                                index: i,
                                time: Instant::now(),
                            });
                        }
                    }

                    if ui.button("add new").clicked() {
                        x.show_new_strategy_window = true;
                    }
                });

            ui.separator();

            match current_kind {
                StrategyKind::StaticTuning => {
                    ui.horizontal(|ui| {
                        show_hide_button(ui, &mut x.show_tuning_editor, "global tuning");
                        show_hide_button(ui, &mut x.show_reference_editor, "reference");
                        show_hide_button(ui, &mut x.show_neighbourhood_editor, "neighbourhoods");
                    });
                }
            }
        });
    }
}

pub struct AsWindows<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);

impl<'a, T: FiveLimitStackType + PartialEq> GuiShow<T> for AsWindows<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        self.display_new_strategy_window(ui, forward);

        let AsWindows(x) = self;
        let ctx = ui.ctx();
        let (current_name, _) = &x.strategies[x.current_strategy];

        if x.show_tuning_editor {
            egui::containers::Window::new(format!("global tuning ({current_name})"))
                .open(&mut x.show_tuning_editor)
                .collapsible(false)
                .show(ctx, |ui| {
                    x.tuning_editor.show(ui, forward);
                });
        }

        if x.show_reference_editor {
            egui::containers::Window::new(format!("reference ({current_name})"))
                .open(&mut x.show_reference_editor)
                .collapsible(false)
                .show(ctx, |ui| {
                    x.reference_editor.show(ui, forward);
                });
        }

        if x.show_neighbourhood_editor {
            egui::containers::Window::new(format!("neighbourhoods ({current_name})"))
                .open(&mut x.show_neighbourhood_editor)
                .collapsible(false)
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
        if x.show_new_strategy_window {
            egui::containers::Window::new("new strategy")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("name:");
                        ui.text_edit_singleline(&mut x.new_strategy_name);
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("clone");
                        ui.vertical(|ui| {
                            egui::ComboBox::from_id_salt("clone strategy picker")
                                .selected_text(x.clone_index.map_or("...", |i| &x.strategies[i].0))
                                .show_ui(ui, |ui| {
                                    for (i, (name, _)) in x.strategies.iter().enumerate() {
                                        if ui
                                            .selectable_value(&mut x.clone_index, Some(i), name)
                                            .clicked()
                                        {
                                            x.template_index = None {};
                                        }
                                    }
                                });
                        });

                        ui.separator();

                        ui.label("template");
                        ui.vertical(|ui| {
                            egui::ComboBox::from_id_salt("template strategy picker")
                                .selected_text(
                                    x.template_index.map_or("...", |i| &x.templates[i].name),
                                )
                                .show_ui(ui, |ui| {
                                    for (i, conf) in x.templates.iter().enumerate() {
                                        if ui
                                            .selectable_value(
                                                &mut x.template_index,
                                                Some(i),
                                                &conf.name,
                                            )
                                            .clicked()
                                        {
                                            x.clone_index = None {};
                                        }
                                    }
                                });
                        });
                    });

                    ui.separator();

                    let finished = !x.new_strategy_name.is_empty()
                        & (x.template_index.is_some() | x.clone_index.is_some());

                    ui.horizontal(|ui| {
                        if ui.add_enabled(finished, egui::Button::new("add")).clicked() {
                            x.clone_index.map(|i| {
                                x.strategies
                                    .push((x.new_strategy_name.clone(), x.strategies[i].1));
                                let _ = forward.send(FromUi::CloneStrategy {
                                    index: i,
                                    time: Instant::now(),
                                });
                            });

                            x.template_index.map(|i| {
                                x.strategies.push((
                                    x.new_strategy_name.clone(),
                                    x.templates[i].strategy_kind(),
                                ));
                                let _ = forward.send(FromUi::AddStrategyFromTemplate {
                                    index: i,
                                    time: Instant::now(),
                                });
                            });

                            x.new_strategy_name.clear();
                            x.clone_index = None {};
                            x.template_index = None {};
                            x.show_new_strategy_window = false;
                        }

                        if ui.button("discard").clicked() {
                            x.new_strategy_name.clear();
                            x.clone_index = None {};
                            x.template_index = None {};
                            x.show_new_strategy_window = false;
                        }
                    });
                });
        }
    }
}
