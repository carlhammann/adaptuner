use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use eframe::egui::{self};

use crate::interval::stacktype::r#trait::StackType;

use super::{common::correction_system_chooser, latticewindow::LatticeWindowControls};

pub fn lattice_control_window<T: StackType>(
    ui: &mut egui::Ui,
    values: &Rc<RefCell<LatticeWindowControls>>,
) {
    let _ = RefMut::map(values.borrow_mut(), |x| {
        ui.horizontal(|ui| {
            correction_system_chooser::<T>(ui, &mut x.correction_system_index);

            ui.separator();

            ui.vertical(|ui| {
                ui.label("show notes around the reference");
                for i in (0..T::num_intervals()).rev() {
                    ui.add(
                        egui::Slider::new(&mut x.background_stack_distances[i], 0..=6)
                            .smart_aim(false)
                            .text(&T::intervals()[i].name),
                    );
                }
            });
        });

        &mut x.correction_system_index // whatever
    });
}

pub fn lattice_zoom_window(ui: &mut egui::Ui, values: &Rc<RefCell<LatticeWindowControls>>) {
    let _ = RefMut::map(values.borrow_mut(), |x| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::widgets::Slider::new(&mut x.zoom, 5.0..=100.0)
                    .smart_aim(false)
                    .show_value(false)
                    .logarithmic(true)
                    .text("zoom"),
            );
        });
        &mut x.zoom // whatever
    });
}

//             ui.separator();
//             if let Some((curr_neighbourhood_name, curr_neighbourhood_index)) =
//                 &self.curr_neighbourhood_name_and_index
//             {
//                 ui.vertical(|ui| {
//                     ui.label(format!(
//                         "neighbourhood {}: \"{}\"",
//                         curr_neighbourhood_index, curr_neighbourhood_name,
//                     ));
//
//                     if ui.button("switch to next neighbourhood").clicked() {
//                         let _ = forward.send(FromUi::NextNeighbourhood {
//                             time: Instant::now(),
//                         });
//                     }
//
//                     if ui.button("delete current neighbourhood").clicked() {
//                         let _ = forward.send(FromUi::DeleteCurrentNeighbourhood {
//                             time: Instant::now(),
//                         });
//                     }
//
//                     ui.horizontal(|ui| {
//                         if ui
//                             .add_enabled(
//                                 !self.new_neighbourhood_name.is_empty(),
//                                 egui::Button::new("add new neighbourhood"),
//                             )
//                             .clicked()
//                         {
//                             let _ = forward.send(FromUi::NewNeighbourhood {
//                                 name: self.new_neighbourhood_name.clone(),
//                             });
//                             self.new_neighbourhood_name.clear();
//                         }
//                         ui.text_edit_singleline(&mut self.new_neighbourhood_name);
//                     });
//                 });
//             }
//         });
//     },
// );
