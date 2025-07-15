use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    config::StrategyKind,
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
};

use super::{
    editor::{
        neighbourhood::NeighbourhoodEditor,
        reference::{ReferenceEditor, ReferenceEditorConfig},
        tuning::{TuningEditor, TuningEditorConfig},
    },
    r#trait::GuiShow,
};

pub struct StrategyWindows<T: StackType> {
    current_strategy: usize,
    strategies: Vec<(String, StrategyKind)>,
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
        tuning_editor: TuningEditorConfig,
        reference_editor: ReferenceEditorConfig,
    ) -> Self {
        Self {
            current_strategy: 0,
            strategies,
            show_tuning_editor: false,
            show_reference_editor: false,
            show_neighbourhood_editor: false,
            tuning_editor: TuningEditor::new(tuning_editor),
            reference_editor: ReferenceEditor::new(reference_editor),
            neigbourhood_editor: NeighbourhoodEditor::new(),
        }
    }

    fn hide_all_windows(&mut self) {
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

pub struct AsStrategyPicker<'a, T: StackType>(pub &'a mut StrategyWindows<T>);

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
                });

            ui.label(":");

            match current_kind {
                StrategyKind::Static => {
                    ui.horizontal(|ui| {
                        ui.toggle_value(&mut x.show_tuning_editor, "global tuning");
                        ui.toggle_value(&mut x.show_reference_editor, "reference");
                        ui.toggle_value(&mut x.show_neighbourhood_editor, "neighbourhoods");
                        // set all other show* variables to false, as they're not relevant to
                        // StrategyKind::Static
                    });
                }
            }
        });
    }
}

pub struct AsWindows<'a, T: StackType>(pub &'a mut StrategyWindows<T>);

impl<'a, T: FiveLimitStackType + PartialEq> GuiShow<T> for AsWindows<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
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
