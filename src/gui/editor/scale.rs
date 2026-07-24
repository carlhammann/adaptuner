use eframe::egui::{self, vec2};

use crate::{
    config::{MelodyStrategyConfig, Named, StrategyConfig},
    gui::{
        common::{show_list_edit, ListEditOpts, ListEditResult},
        r#trait::{ReceiveToUiRef, UiAdaptor},
    },
    interval::stacktype::r#trait::StackType,
    msg::ToUi,
    neighbourhood::{Neighbourhood, SomeCompleteNeighbourhood},
    strategy::{
        melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
    util::list_action::ListAction,
};

pub struct ScaleEditor {
    current_scale_index: usize,
}

impl ScaleEditor {
    pub fn new() -> Self {
        Self {
            current_scale_index: 0,
        }
    }
}

pub enum ScaleEditorResult {
    NoChange,
    ChangeList,
    Select(usize),
    ChangeScale(usize),
}

impl ScaleEditor {
    pub fn show<T: StackType>(
        &mut self,
        ui: &mut egui::Ui,
        scales: &mut Vec<Named<SomeCompleteNeighbourhood<T>>>,
    ) -> ScaleEditorResult {
        let list_edit_res = show_list_edit(
            ui,
            "neighbourhood editor",
            scales,
            Some(self.current_scale_index),
            ListEditOpts {
                empty_allowed: false,
                select_allowed: true,
                no_selection_allowed: false,
                delete_allowed: true,
                reorder_allowed: true,
                show_one: Box::new(|ui, i, elem| {
                    ui.add(egui::TextEdit::singleline(&mut elem.name).min_size(vec2(
                        ui.style().spacing.text_edit_width / 2.0,
                        ui.style().spacing.interact_size.y,
                    )));

                    let mut msg: Option<usize> = None {};
                    if T::num_temperaments() > 0 {
                        egui::ComboBox::from_id_salt(format!("temperament picker for scale {i}"))
                            .selected_text("apply temperament")
                            .show_ui(ui, |ui| {
                                for (j, t) in T::temperaments().iter().enumerate() {
                                    if ui.button(&t.name).clicked() {
                                        elem.named.for_each_stack_mut(|_, stack| {
                                            stack.apply_temperament(j);
                                        });
                                        msg = Some(i);
                                    }
                                }

                                ui.separator();

                                if ui.button("no temperament").clicked() {
                                    elem.named.for_each_stack_mut(|_, stack| {
                                        stack.make_pure();
                                    });
                                    msg = Some(i);
                                }
                            });
                    }
                    msg
                }),
                clone: Some(Box::new(|ui, _elems, selected| {
                    ui.separator();
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
            ListEditResult::Message(i) => ScaleEditorResult::ChangeScale(i),
            ListEditResult::Action(action) => {
                action.apply_to(
                    scales,
                    self.current_scale_index,
                    |x| x.clone(),
                    |new_scale_index| {
                        self.current_scale_index = new_scale_index;
                    },
                );
                match action {
                    ListAction::Select(i) => ScaleEditorResult::Select(i),
                    ListAction::Delete(_) | ListAction::Clone(_) | ListAction::SwapWithPrev(_) => {
                        ScaleEditorResult::ChangeList
                    }
                    ListAction::Deselect => unreachable!(),
                }
            }
            ListEditResult::None => ScaleEditorResult::NoChange,
        }
    }
}

impl<T: StackType, A: UiAdaptor<T>> ReceiveToUiRef<T, A> for ScaleEditor {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, adaptor: &A) {
        match msg {
            ToUi::SelectScale { index } => {
                self.current_scale_index = *index;
            }
            ToUi::Consider { stack } => {
                match &mut adaptor.strategy_config_mut()[adaptor.active_strategy_index()] {
                    StrategyConfig::TwoStep {
                        melody:
                            MelodyStrategyConfig::StaticNeighbourhoods(
                                StaticNeighbourhoodsAsMelodyConfig { scales, .. },
                            ),
                        ..
                    }
                    | StrategyConfig::StaticNeighbourhoods {
                        config: StaticNeighbourhoodsConfig { scales, .. },
                        ..
                    } => {
                        let _ = scales[self.current_scale_index].named.insert(stack);
                    }
                }
            }
            _ => {}
        }
    }
}
