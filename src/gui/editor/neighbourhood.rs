use std::{marker::PhantomData, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    gui::common::{ListEdit, ListEditOpts, RefListEdit},
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, ReceiveMsgRef, ToUi},
};

pub struct NeighbourhoodEditor<T: StackType> {
    _phantom: PhantomData<T>,
    current_neighbourhood_index: Option<usize>,
}

impl<T: StackType> NeighbourhoodEditor<T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            current_neighbourhood_index: None {},
        }
    }
}

impl<T: StackType> NeighbourhoodEditor<T> {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        names: &mut Vec<String>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let mut listedit = RefListEdit::new(names, &mut self.current_neighbourhood_index);
        let list_edit_res = listedit.show(
            ui,
            "neighbourhood editor",
            ListEditOpts {
                empty_allowed: false,
                select_allowed: true,
                no_selection_allowed: false,
                delete_allowed: true,
                reorder_allowed: true,
                show_one: Box::new(|ui, i, elem, _| {
                    ui.add(egui::TextEdit::singleline(elem).min_size(vec2(
                        ui.style().spacing.text_edit_width / 2.0,
                        ui.style().spacing.interact_size.y,
                    )));

                    let mut msg: Option<FromUi<T>> = None {};
                    if T::num_temperaments() > 0 {
                        egui::ComboBox::from_id_salt(format!("temperament picker {i}"))
                            .selected_text("apply temperament")
                            .show_ui(ui, |ui| {
                                for (j, t) in T::temperaments().iter().enumerate() {
                                    if ui.button(&t.name).clicked() {
                                        msg = Some(FromUi::ApplyTemperamentToNeighbourhood {
                                            time: Instant::now(),
                                            temperament: j,
                                            neighbourhood: i,
                                        });
                                    }
                                }

                                ui.separator();

                                if ui.button("no temperament").clicked() {
                                    msg = Some(FromUi::MakeNeighbourhoodPure {
                                        time: Instant::now(),
                                        neighbourhood: i,
                                    });
                                }
                            });
                    }
                    msg
                }),
                clone: Some(Box::new(|ui, _elems, selected, _| {
                    ui.separator();
                    if let Some(i) = selected {
                        if ui.button("create copy of selected").clicked() {
                            // let _ = forward.send(FromUi::Action {
                            //     action: StrategyAction::SwitchToNeighbourhood(i),
                            //     time: Instant::now(),
                            // });
                            Some(i)
                        } else {
                            None {}
                        }
                    } else {
                        None {}
                    }
                })),
            },
            &mut (),
        );

        match list_edit_res {
            crate::gui::common::ListEditResult::Message(message) => {
                let _ = forward.send(message);
            }
            crate::gui::common::ListEditResult::Action(action) => {
                let _ = forward.send(FromUi::NeighbourhoodListAction {
                    action,
                    time: Instant::now(),
                });
            }
            crate::gui::common::ListEditResult::None => {}
        }
    }
}

impl<T: StackType> ReceiveMsgRef<ToUi<T>> for NeighbourhoodEditor<T> {
    fn receive_msg_ref(&mut self, msg: &ToUi<T>) {
        match msg {
            ToUi::CurrentNeighbourhoodIndex { index } => {
                self.current_neighbourhood_index = Some(*index);
            }
            _ => {}
        }
    }
}
