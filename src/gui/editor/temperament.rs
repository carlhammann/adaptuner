use eframe::egui::{self, vec2};
use ndarray::{Array2, ArrayView1};

use crate::{
    gui::common::{ListEdit, ListEditOpts, RefListEdit},
    interval::{
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
        temperament::TemperamentDefinition,
    },
};

#[derive(Clone)]
struct MaybeRealized<T: IntervalBasis> {
    definition: TemperamentDefinition<T>,
    realized: bool,
    determinate: bool,
    recompute: bool,
}

pub struct TemperamentEditor<T: IntervalBasis> {
    definitions: Vec<MaybeRealized<T>>,
}

impl<T: StackType> TemperamentEditor<T> {
    pub fn new() -> Self {
        Self {
            definitions: T::temperament_definitions()
                .iter()
                .map(|definition| MaybeRealized {
                    definition: definition.clone(),
                    realized: true,
                    determinate: true,
                    recompute: false,
                })
                .collect(),
        }
    }
}

impl<T: IntervalBasis> MaybeRealized<T> {
    fn show(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::TextEdit::singleline(&mut self.definition.name).min_size(vec2(
                ui.style().spacing.text_edit_width / 2.0,
                ui.style().spacing.interact_size.y,
            )),
        );

        ui.vertical(|ui| {
            for i in 0..T::num_intervals() {
                ui.horizontal(|ui| {
                    for j in 0..T::num_intervals() {
                        self.recompute |= ui
                            .add(egui::DragValue::new(&mut self.definition.tempered[(i, j)]))
                            .on_hover_text_at_pointer(format!(
                                "number of tempered {}s",
                                T::intervals()[j].name
                            ))
                            .changed();
                    }
                    ui.add(egui::Label::new("=").halign(egui::Align::Max));
                    for j in 0..T::num_intervals() {
                        self.recompute |= ui
                            .add(egui::DragValue::new(&mut self.definition.pure[(i, j)]))
                            .on_hover_text_at_pointer(format!(
                                "number of pure {}s",
                                T::intervals()[j].name
                            ))
                            .changed();
                    }
                });
            }
            let key_distance_from_coeffs = |coeffs: ArrayView1<StackCoeff>| {
                coeffs.iter().enumerate().fold(0, |acc, (i, c)| {
                    acc + *c * T::intervals()[i].key_distance as StackCoeff
                })
            };

            self.realized = true;
            for i in 0..T::num_intervals() {
                let tempered_keys = key_distance_from_coeffs(self.definition.tempered.row(i));
                let pure_keys = key_distance_from_coeffs(self.definition.pure.row(i));
                if tempered_keys != pure_keys {
                    self.realized = false;
                    ui.label(
                        egui::RichText::new(format!(
                            "equation {}: tempered side spans \
                            {tempered_keys}, pure side {pure_keys} keys",
                            i + 1
                        ))
                        .color(ui.style().visuals.warn_fg_color),
                    );
                }
            }
            if self.recompute {
                self.determinate = self.definition.realize().is_ok();
                self.realized &= self.determinate;
                self.recompute = false;
            }

            if !self.determinate {
                ui.label(
                    egui::RichText::new("The equations form an indeterminate system")
                        .color(ui.style().visuals.warn_fg_color),
                );
            }
        });
    }
}

impl<T: StackType> TemperamentEditor<T> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Vec<TemperamentDefinition<T>>> {
        egui::ScrollArea::vertical()
            .max_height(ui.ctx().available_rect().height() / 2.0)
            .show(ui, |ui| {
                let mut dummy = None {};
                RefListEdit::new(&mut self.definitions, &mut dummy).show(
                    ui,
                    "temperament_list_edit",
                    ListEditOpts {
                        empty_allowed: true,
                        select_allowed: false,
                        no_selection_allowed: false,
                        delete_allowed: true,
                        reorder_allowed: true,
                        show_one: Box::new(|ui, _i, t, _| {
                            t.show(ui);
                            None::<()> {}
                        }),
                        clone: None {},
                    },
                    &mut (),
                );
            });

        if ui.button("add new temperament").clicked() {
            let n = T::num_intervals();
            let definition = TemperamentDefinition::new(
                "new temperament".into(),
                Array2::zeros((n, n)),
                Array2::zeros((n, n)),
            );
            self.definitions.push(MaybeRealized {
                realized: false,
                determinate: false,
                definition,
                recompute: false,
            });
        }

        ui.separator();

        if ui
            .add_enabled(
                self.definitions.iter().all(|d| d.realized),
                egui::Button::new("update temperaments"),
            )
            .clicked()
        {
            return Some(
                self.definitions
                    .iter()
                    .map(|d| d.definition.clone())
                    .collect(),
            );
        }
        None {}
    }
}
