use std::{marker::PhantomData, time::Instant, sync::mpsc};

use eframe::egui;

use crate::{
    interval::stacktype::r#trait::{IntervalBasis, StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
};

use super::r#trait::GuiShow;

pub struct StaticControlWindow<T: IntervalBasis> {
    _phantom: PhantomData<T>,
    curr_neighbourhood_name_and_index: Option<(String, usize)>,
    new_neighbourhood_name: String,
}

impl<T: IntervalBasis> StaticControlWindow<T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            curr_neighbourhood_name_and_index: None {},
            new_neighbourhood_name: String::with_capacity(64),
        }
    }
}

impl<T: StackType> GuiShow<T> for StaticControlWindow<T> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some((curr_neighbourhood_name, curr_neighbourhood_index)) =
            &self.curr_neighbourhood_name_and_index
        {
            ui.vertical(|ui| {
                ui.label(format!(
                    "neighbourhood {}: \"{}\"",
                    curr_neighbourhood_index, curr_neighbourhood_name,
                ));

                if ui.button("switch to next neighbourhood").clicked() {
                    let _ = forward.send(FromUi::NextNeighbourhood {
                        time: Instant::now(),
                    });
                }

                if ui.button("delete current neighbourhood").clicked() {
                    let _ = forward.send(FromUi::DeleteCurrentNeighbourhood {
                        time: Instant::now(),
                    });
                }

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.new_neighbourhood_name.is_empty(),
                            egui::Button::new("add new neighbourhood"),
                        )
                        .clicked()
                    {
                        let _ = forward.send(FromUi::NewNeighbourhood {
                            name: self.new_neighbourhood_name.clone(),
                        });
                        self.new_neighbourhood_name.clear();
                    }
                    ui.text_edit_singleline(&mut self.new_neighbourhood_name);
                });
            });
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for StaticControlWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentNeighbourhoodName { index, name } => {
                if let Some((old_name, old_index)) = &mut self.curr_neighbourhood_name_and_index {
                    if index != old_index {
                        *old_index = *index;
                        old_name.clone_from(name);
                    }
                } else {
                    self.curr_neighbourhood_name_and_index = Some((name.clone(), *index));
                }
            }
            _ => {}
        }
    }
}
