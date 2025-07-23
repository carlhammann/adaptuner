use std::{marker::PhantomData, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    gui::{
        common::{ListEdit, ListEditOpts},
        r#trait::GuiShow,
    },
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
    util::list_action::ListAction,
};

pub struct NeighbourhoodEditor<T: StackType> {
    _phantom: PhantomData<T>,
    names: ListEdit<String>,
}

impl<T: StackType> NeighbourhoodEditor<T> {
    pub fn new(names: Vec<String>) -> Self {
        Self {
            _phantom: PhantomData,
            names: ListEdit::new(names),
        }
    }

    pub fn get_all(&self) -> &[String] {
        self.names.get_all()
    }

    pub fn set_all(&mut self, names: &[String]) {
        self.names.set_all(names);
    }
}

impl<T: StackType> GuiShow<T> for NeighbourhoodEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let list_edit_res = self.names.show(
            ui,
            "neighbourhood editor",
            ListEditOpts {
                empty_allowed: false,
                select_allowed: true,
                no_selection_allowed: false,
                delete_allowed: true,
                show_one: Box::new(|ui, i, elem| {
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
                clone: Some(Box::new(|ui, _elems, selected| {
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

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for NeighbourhoodEditor<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentNeighbourhoodIndex { index } => {
                self.names.apply(ListAction::Select(*index));
                // self.curr = Some(IndexAndName {
                //     name: name.clone(),
                //     index: *index,
                //     n_neighbourhoods: *n_neighbourhoods,
                // });
            }

            _ => {}
        }
    }
}
