use eframe::egui::{self, vec2};
use ndarray::{Array1, Array2};

use crate::{
    gui::common::{rational_drag_value, show_list_edit, ListEditOpts, ListEditResult},
    interval::stacktype::r#trait::{CoordinateSystem, IntervalBasis, NamedInterval, StackType},
    util::subsequences::Subsequences,
};

pub struct CommaEditor<T: IntervalBasis> {
    commas: Vec<NamedInterval<T>>,
    possible_bases: Vec<Vec<usize>>,
    tmp_str: String,
}

fn compute_possible_bases<T: IntervalBasis>(intervals: &[NamedInterval<T>]) -> Vec<Vec<usize>> {
    let mut res = vec![];
    let mut subsequences = Subsequences::new(intervals.len(), T::num_intervals());
    while let Some(basis_indices) = subsequences.next() {
        let basis_columnwise =
            Array2::from_shape_fn((T::num_intervals(), T::num_intervals()), |(i, j)| {
                intervals[basis_indices[j]].coeffs[i]
            });
        if CoordinateSystem::new(basis_columnwise).is_ok() {
            res.push(basis_indices.into());
        }
    }
    res
}

impl<T: StackType> CommaEditor<T> {
    pub fn new() -> Self {
        Self {
            commas: T::named_intervals().clone(),
            possible_bases: compute_possible_bases(&*T::named_intervals()),
            tmp_str: String::with_capacity(3 * T::num_intervals()),
        }
    }
}

impl<T: IntervalBasis> NamedInterval<T> {
    fn show(&mut self, ui: &mut egui::Ui, i: usize) -> bool {
        let mut changed = false;
        ui.add(egui::TextEdit::singleline(&mut self.name).min_size(vec2(
            ui.style().spacing.text_edit_width / 2.0,
            ui.style().spacing.interact_size.y,
        )));
        ui.add(
            egui::TextEdit::singleline(&mut self.short_name)
                .char_limit(2)
                .min_size(vec2(
                    ui.style().spacing.interact_size.x,
                    ui.style().spacing.interact_size.y,
                )),
        );
        let id = egui::Id::new(format!("comma {i}"));
        ui.vertical(|ui| {
            for (i, c) in self.coeffs.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= rational_drag_value(ui, id.with(i), c);
                    ui.label(&T::intervals()[i].name);
                });
            }
        });
        changed
    }
}

impl<T: StackType> CommaEditor<T> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Vec<NamedInterval<T>>> {
        let mut changed = false;
        egui::ScrollArea::vertical()
            .max_height(ui.ctx().available_rect().height() / 2.0)
            .show(ui, |ui| {
                let res = show_list_edit(
                    ui,
                    "comma_list_edit",
                    &mut self.commas,
                    None,
                    ListEditOpts {
                        empty_allowed: true,
                        select_allowed: false,
                        no_selection_allowed: false,
                        delete_allowed: true,
                        reorder_allowed: true,
                        show_one: Box::new(|ui, i, t| {
                            if t.show(ui, i) {
                                Some(true)
                            } else {
                                None {}
                            }
                        }),
                        clone: None {},
                    },
                );
                match res {
                    ListEditResult::Action(a) => {
                        a.apply_to_no_select(&mut self.commas, |x| x.clone());
                        changed = true;
                    }
                    ListEditResult::Message(b) => changed = b,
                    ListEditResult::None => {}
                }
            });

        if ui.button("add new comma").clicked() {
            self.commas.push(NamedInterval::new(
                Array1::zeros(T::num_intervals()),
                "new comma".into(),
                "x".into(),
            ));
        }

        if changed {
            self.possible_bases = compute_possible_bases(&self.commas);
        }
        if self.possible_bases.is_empty() {
            ui.label("These commas won't be usable in note names, because no basis can be formed.");
        } else {
            ui.collapsing(
                "Possible bases with these commas, in the order in wich they will be used",
                |ui| {
                    for b in &self.possible_bases {
                        self.tmp_str.clear();
                        for i in b {
                            self.tmp_str.push_str(&self.commas[*i].short_name);
                            self.tmp_str.push(' ');
                        }
                        ui.label(&self.tmp_str);
                    }
                },
            );
        }

        ui.separator();

        if ui.button("update commas").clicked() {
            return Some(self.commas.clone());
        }
        None {}
    }
}
