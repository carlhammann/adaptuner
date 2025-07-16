use std::{marker::PhantomData, sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    gui::r#trait::GuiShow,
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
    strategy::r#trait::StrategyAction,
};

struct IndexAndName {
    index: usize,
    n_neighbourhoods: usize,
    name: String,
}

pub struct NeighbourhoodEditor<T: StackType> {
    _phantom: PhantomData<T>,
    curr: Option<IndexAndName>,
    new_neighbourhood_name: String,
}

impl<T: StackType> NeighbourhoodEditor<T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            curr: None {},
            new_neighbourhood_name: String::with_capacity(64),
        }
    }
}

impl<T: StackType> GuiShow<T> for NeighbourhoodEditor<T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(IndexAndName {
            index: curr_neighbourhood_index,
            n_neighbourhoods,
            name: curr_neighbourhood_name,
        }) = &self.curr
        {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(format!(
                        "current ({}/{}): ",
                        curr_neighbourhood_index + 1,
                        n_neighbourhoods,
                    ));
                    ui.strong(curr_neighbourhood_name);
                });

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(*n_neighbourhoods > 1, egui::Button::new("previous"))
                        .clicked()
                    {
                        let _ = forward.send(FromUi::Action {
                            action: StrategyAction::PrevNeighbourhood,
                            time: Instant::now(),
                        });
                    }

                    if ui
                        .add_enabled(*n_neighbourhoods > 1, egui::Button::new("next"))
                        .clicked()
                    {
                        let _ = forward.send(FromUi::Action {
                            action: StrategyAction::NextNeighbourhood,
                            time: Instant::now(),
                        });
                    }
                });

                ui.separator();

                if T::num_temperaments() > 0 {
                    ui.label(format!(
                        "apply temperament to all notes in \"{curr_neighbourhood_name}\":"
                    ));
                    for (i, t) in T::temperaments().iter().enumerate() {
                        if ui.button(&t.name).clicked() {
                            let _ = forward.send(FromUi::ApplyTemperamentToCurrentNeighbourhood {
                                time: Instant::now(),
                                temperament: i,
                            });
                        }
                    }
                    if ui.button("no temperament").clicked() {
                        let _ = forward.send(FromUi::MakeCurrentNeighbourhoodPure {
                            time: Instant::now(),
                        });
                    }
                    ui.separator();
                }

                ui.label(format!("new copy of \"{curr_neighbourhood_name}\""));

                ui.horizontal(|ui| {
                    ui.label("name:");
                    ui.text_edit_singleline(&mut self.new_neighbourhood_name);
                });

                if ui
                    .add_enabled(
                        !self.new_neighbourhood_name.is_empty(),
                        egui::Button::new("create"),
                    )
                    .clicked()
                {
                    let _ = forward.send(FromUi::NewNeighbourhood {
                        name: self.new_neighbourhood_name.clone(),
                    });
                    self.new_neighbourhood_name.clear();
                }

                ui.separator();

                if ui
                    .add_enabled(
                        *n_neighbourhoods > 1,
                        egui::Button::new(format!("delete \"{curr_neighbourhood_name}\"")),
                    )
                    .clicked()
                {
                    let _ = forward.send(FromUi::DeleteCurrentNeighbourhood {
                        time: Instant::now(),
                    });
                }
            });
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for NeighbourhoodEditor<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentNeighbourhoodName {
                index,
                n_neighbourhoods,
                name,
            } => {
                self.curr = Some(IndexAndName {
                    name: name.clone(),
                    index: *index,
                    n_neighbourhoods: *n_neighbourhoods,
                });
            }

            _ => {}
        }
    }
}
