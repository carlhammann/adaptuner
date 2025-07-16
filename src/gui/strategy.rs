use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    bindable::Bindable,
    config::{ExtendedStrategyConfig, StrategyKind},
    interval::stacktype::r#trait::{FiveLimitStackType, IntervalBasis, StackType},
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
    curr_strategy_index: Option<usize>,
    strategies: Vec<ExtendedStrategyConfig<T>>,

    show_new_strategy_window: bool,
    templates: &'static [ExtendedStrategyConfig<T>],
    clone_index: Option<usize>,
    template_index: Option<usize>,
    new_strategy_name: String,
    new_strategy_description: String,

    show_delete_strategy_window: bool,
    delete_index: Option<usize>,

    show_tuning_editor: bool,
    tuning_editor: TuningEditor<T>,

    show_reference_editor: bool,
    reference_editor: ReferenceEditor<T>,

    show_neighbourhood_editor: bool,
    neigbourhood_editor: NeighbourhoodEditor<T>,

    show_binding_editor: bool,
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
            templates,
            show_new_strategy_window: false,
            clone_index: None {},
            template_index: None {},
            new_strategy_name: String::with_capacity(32),
            new_strategy_description: String::with_capacity(128),
            show_delete_strategy_window: false,
            delete_index: None {},
            show_tuning_editor: false,
            show_reference_editor: false,
            show_neighbourhood_editor: false,
            tuning_editor: TuningEditor::new(tuning_editor),
            reference_editor: ReferenceEditor::new(reference_editor),
            neigbourhood_editor: NeighbourhoodEditor::new(),
            show_binding_editor: false,
        }
    }

    fn hide_all_windows(&mut self) {
        // self.show_new_strategy_window = false;
        // self.show_delete_strategy_window = false;
        self.show_tuning_editor = false;
        self.show_reference_editor = false;
        self.show_neighbourhood_editor = false;
        self.show_binding_editor = false;
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for StrategyWindows<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SwitchToStrategy { index } => {
                self.curr_strategy_index = Some(*index);
            }
            ToUi::DeleteStrategy { index } => {
                if self.curr_strategy_index == Some(*index) {
                    self.hide_all_windows();
                }
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
        ui.horizontal(|ui| {
            if ui.button("new strategy").clicked() {
                x.show_new_strategy_window = true;
            }

            if ui
                .add_enabled(
                    x.strategies.len() > 1,
                    egui::Button::new("delete a strategy"),
                )
                .clicked()
            {
                x.show_delete_strategy_window = true;
            }

            ui.separator();

            if strategy_picker(
                ui,
                "strategy picker",
                &mut x.curr_strategy_index,
                &x.strategies,
            ) {
                let _ = forward.send(FromUi::SwitchToStrategy {
                    index: x.curr_strategy_index.unwrap(), // this is safe, because the strategy_picker returned true
                    time: Instant::now(),
                });
            }

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
        self.display_delete_strategy_window(ui, forward);
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
                .open(&mut x.show_tuning_editor)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    x.tuning_editor.show(ui, forward);
                });
        }

        if x.show_reference_editor {
            egui::containers::Window::new(format!("reference ({current_name})"))
                .open(&mut x.show_reference_editor)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    x.reference_editor.show(ui, forward);
                });
        }

        if x.show_neighbourhood_editor {
            egui::containers::Window::new(format!("neighbourhoods ({current_name})"))
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
        if x.show_new_strategy_window {
            egui::containers::Window::new("new strategy")
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

    fn display_delete_strategy_window(
        &mut self,
        ui: &mut egui::Ui,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let AsWindows(x) = self;
        let ctx = ui.ctx();
        egui::containers::Window::new("delete a strategy")
            .collapsible(false)
            .resizable(false)
            .open(&mut x.show_delete_strategy_window)
            .show(ctx, |ui| {
                strategy_picker(
                    ui,
                    "delete strategy picker",
                    &mut x.delete_index,
                    &x.strategies,
                );

                ui.separator();

                if ui
                    .add_enabled(
                        x.delete_index.is_some() & (x.strategies.len() > 1),
                        egui::Button::new("delete"),
                    )
                    .clicked()
                {
                    let _ = forward.send(FromUi::DeleteStrategy {
                        index: x.delete_index.unwrap(),
                        time: Instant::now(),
                    });
                    x.delete_index = None {};
                }
            });
    }

    fn display_binding_window(&mut self, ui: &mut egui::Ui, _forward: &mpsc::Sender<FromUi<T>>) {
        let AsWindows(x) = self;
        let ctx = ui.ctx();
        // let ExtendedStrategyConfig {
        //     name: current_name,
        //     description,
        //     config: current_config,
        //     bindings: current_bindings,
        // } =

        if x.curr_strategy_index.is_none() {
            return;
        }
        let curr_strategy_index = x.curr_strategy_index.unwrap();
        let current = &mut x.strategies[curr_strategy_index];

        egui::containers::Window::new(format!("bindings ({})", current.name))
            .collapsible(false)
            .resizable(false)
            .open(&mut x.show_binding_editor)
            .show(ctx, |ui| {
                egui::Grid::new("binding seletor grid").show(ui, |ui| {
                    for bindable in [
                        Bindable::SostenutoPedalDown,
                        Bindable::SostenutoPedalUp,
                        Bindable::SoftPedalDown,
                        Bindable::SoftPedalUp,
                    ] {
                        ui.label(format!("{bindable}:"));
                        egui::ComboBox::from_id_salt(bindable)
                            .selected_text(
                                current
                                    .bindings
                                    .get(bindable)
                                    .map_or("--".into(), |action| format!("{action}")),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    current.bindings.get_mut(bindable),
                                    None {},
                                    "--",
                                );
                                for action in current.strategy_kind().allowed_actions() {
                                    ui.selectable_value(
                                        current.bindings.get_mut(bindable),
                                        Some(action.clone()),
                                        &format!("{action}"),
                                    );
                                }
                            });
                        ui.end_row();
                    }
                });
            });
    }
}
