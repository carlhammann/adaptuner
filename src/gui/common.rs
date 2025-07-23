use eframe::egui;
use num_rational::Ratio;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{StackCoeff, StackType},
    },
    notename::correction::Correction,
    util::list_action::ListAction,
};

fn show_list_picker<'a, X>(
    elems: &'a [X],
    selected: &mut Option<usize>,
    ui: &mut egui::Ui,
    elem_name: impl Fn(&X) -> &str,
    elem_description: impl Fn(&X) -> Option<&str>,
) -> Option<(usize, &'a X)> {
    let mut new_selection = None {};
    // egui::ComboBox::from_id_salt(id_salt)
    //     .selected_text(selected.map_or("", |i| elem_name(&elems[i])))
    // .show_ui(ui, |ui| {
    // ui.shrink_width_to_current();
    for (i, elem) in elems.iter().enumerate() {
        let old_selected = selected.clone();
        let r = ui.selectable_value(selected, Some(i), elem_name(elem));
        if r.clicked() {
            if Some(i) != old_selected {
                new_selection = Some((i, elem));
            }
        }
        if let Some(description) = elem_description(elem) {
            r.on_hover_text_at_pointer(description);
        }
    }
    // });
    new_selection
}

pub struct ListEdit<X> {
    elems: Vec<X>,
    selected: Option<usize>,
}

pub struct ListEditOpts<X, M> {
    pub empty_allowed: bool,
    pub select_allowed: bool,
    pub no_selection_allowed: bool,
    pub delete_allowed: bool,
    pub show_one: Box<dyn Fn(&mut egui::Ui, usize, &mut X) -> Option<M>>,
    pub clone: Option<Box<dyn FnOnce(&mut egui::Ui, &[X], Option<usize>) -> Option<usize>>>,
}

pub enum ListEditResult<M> {
    Message(M),
    Action(ListAction),
    None,
}

impl<X> ListEdit<X> {
    pub fn new(elems: Vec<X>) -> Self {
        Self {
            elems,
            selected: None {},
        }
    }

    pub fn select(mut self, i: usize) -> Self {
        self.selected = Some(i);
        self
    }

    pub fn get_all(&self) -> &[X] {
        &self.elems
    }

    pub fn set_all(&mut self, elems: &[X])
    where
        X: Clone,
    {
        self.elems = elems.into();
    }

    pub fn apply(&mut self, action: ListAction)
    where
        X: Clone,
    {
        action.apply_to(|x| x.clone(), &mut self.elems, &mut self.selected);
    }

    pub fn current_selected(&self) -> Option<&X> {
        self.selected.map(|i| &self.elems[i])
    }

    pub fn current_selected_mut(&mut self) -> Option<&mut X> {
        self.selected.map(|i| &mut self.elems[i])
    }

    /// will return the new selection index, if it changed
    pub fn show_as_list_picker(
        &mut self,
        ui: &mut egui::Ui,
        elem_name: impl Fn(&X) -> &str,
        elem_description: impl Fn(&X) -> Option<&str>,
    ) -> Option<(usize, &X)> {
        show_list_picker(
            &self.elems,
            &mut self.selected,
            ui,
            elem_name,
            elem_description,
        )
    }

    fn show_dont_handle<M>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M>,
    ) -> ListEditResult<M> {
        let mut res = ListEditResult::None;
        let mut update_res = |new_res: ListEditResult<M>| match res {
            ListEditResult::None => {
                res = new_res;
            }
            _ => {}
        };
        let selected = self.selected;
        egui::Grid::new(id_salt)
            .min_col_width(ui.style().spacing.interact_size.y)
            .with_row_color(move |i, style| {
                if Some(i) == selected {
                    Some(style.visuals.selection.bg_fill)
                } else {
                    None {}
                }
            })
            .show(ui, |ui| {
                let n = self.elems.len();
                for (i, elem) in self.elems.iter_mut().enumerate() {
                    if opts.select_allowed {
                        let is_current = self.selected == Some(i);
                        if ui.radio(is_current, "").clicked() {
                            if !is_current {
                                update_res(ListEditResult::Action(ListAction::Select(i)));
                            } else {
                                if opts.no_selection_allowed {
                                    update_res(ListEditResult::Action(ListAction::Deselect));
                                }
                            }
                        }
                    }

                    if let Some(m) = (opts.show_one)(ui, i, elem) {
                        update_res(ListEditResult::Message(m));
                    }

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if ui
                            .add_enabled(
                                i > 0,
                                egui::Button::new("⏶").corner_radius(egui::CornerRadius {
                                    ne: 0,
                                    nw: ui.style().visuals.menu_corner_radius.nw,
                                    se: 0,
                                    sw: ui.style().visuals.menu_corner_radius.sw,
                                }),
                            )
                            .clicked()
                        {
                            update_res(ListEditResult::Action(ListAction::SwapWithPrev(i)));
                        }
                        if ui
                            .add_enabled(
                                i < n - 1,
                                egui::Button::new("⏷").corner_radius(egui::CornerRadius {
                                    nw: 0,
                                    ne: ui.style().visuals.menu_corner_radius.ne,
                                    sw: 0,
                                    se: ui.style().visuals.menu_corner_radius.se,
                                }),
                            )
                            .clicked()
                        {
                            update_res(ListEditResult::Action(ListAction::SwapWithPrev(i + 1)));
                        }
                    });

                    if opts.delete_allowed {
                        if ui
                            .add_enabled(opts.empty_allowed || n > 1, egui::Button::new("delete"))
                            .clicked()
                        {
                            update_res(ListEditResult::Action(ListAction::Delete(i)));
                        }
                    }

                    ui.end_row();
                }
            });

        if let Some(f) = opts.clone {
            ui.separator();
            if let Some(i) = f(ui, &self.elems, self.selected) {
                update_res(ListEditResult::Action(ListAction::Clone(i)));
            }
        }

        res
    }

    pub fn show<M>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M>,
    ) -> ListEditResult<M>
    where
        X: Clone,
    {
        let res = self.show_dont_handle(ui, id_salt, opts);
        if let ListEditResult::Action(action) = &res {
            action.apply_to(|x| x.clone(), &mut self.elems, &mut self.selected);
        }
        res
    }
}

pub struct SmallFloatingWindow {
    id: egui::Id,
    open: bool,
    bring_to_foreground: bool,
}

impl SmallFloatingWindow {
    pub fn new(id: egui::Id) -> Self {
        Self {
            id,
            open: false,
            bring_to_foreground: false,
        }
    }

    pub fn show<R>(
        &mut self,
        title: &str,
        ctx: &egui::Context,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        if self.bring_to_foreground {
            let layer_id = egui::LayerId::new(egui::Order::Middle, self.id);
            ctx.move_to_top(layer_id);
            self.bring_to_foreground = false;
        }

        egui::containers::Window::new(title)
            .id(self.id)
            .collapsible(false)
            .resizable(false)
            .open(&mut self.open)
            .show(ctx, add_contents)
    }

    pub fn show_hide_button(&mut self, ui: &mut egui::Ui, description: &str) -> bool {
        show_hide_button(
            ui,
            description,
            &mut self.open,
            &mut self.bring_to_foreground,
        )
    }
}

pub fn show_hide_button(
    ui: &mut egui::Ui,
    description: &str,
    open: &mut bool,
    bring_to_foreground: &mut bool,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if ui
            .add(
                egui::Button::new(description).corner_radius(egui::CornerRadius {
                    ne: 0,
                    nw: ui.style().visuals.menu_corner_radius.nw,
                    se: 0,
                    sw: ui.style().visuals.menu_corner_radius.sw,
                }),
            )
            .clicked()
        {
            *open = true;
            *bring_to_foreground = true;
            clicked = true;
        }

        if ui
            .add_enabled(
                *open,
                egui::Button::new("x").corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: ui.style().visuals.menu_corner_radius.ne,
                    sw: 0,
                    se: ui.style().visuals.menu_corner_radius.se,
                }),
            )
            .clicked()
        {
            *open = false;
            clicked = true;
        }
    });
    clicked
}

pub fn correction_system_chooser<T: StackType>(
    ui: &mut egui::Ui,
    system_index: &mut usize,
    use_cent_values: &mut bool,
) {
    ui.vertical(|ui| {
        ui.checkbox(use_cent_values, "write temperaments as cent values");
        if T::correction_systems().len() > 0 {
            ui.label("edit temperaments as fractions of");
            for (i, system) in T::correction_systems().iter().enumerate() {
                ui.selectable_value(system_index, i, &system.name);
            }
        }
    });
}

/// returns true iff the number changed
pub fn rational_drag_value(ui: &mut egui::Ui, id: egui::Id, value: &mut Ratio<StackCoeff>) -> bool {
    let numer_id = id.with("numer");
    let denom_id = id.with("denom");

    let mut numer = ui
        .data(|map| map.get_temp(numer_id))
        .unwrap_or(*value.numer());
    let mut denom = ui
        .data(|map| map.get_temp(denom_id))
        .unwrap_or(*value.denom());

    let numer_response = ui.add(egui::DragValue::new(&mut numer));
    if numer_response.changed() {
        ui.data_mut(|map| map.insert_temp(numer_id, numer));
    }
    ui.label("/");
    let denom_response = ui.add(egui::DragValue::new(&mut denom));
    if denom_response.changed() {
        ui.data_mut(|map| map.insert_temp(denom_id, denom));
    }

    let finished = |r: &egui::Response| r.lost_focus() | r.drag_stopped();
    let started = |r: &egui::Response| r.gained_focus() | r.drag_started();

    if (finished(&denom_response) & !started(&numer_response))
        | (finished(&numer_response) & !started(&denom_response))
    {
        let new_numer = ui
            .data_mut(|map| map.remove_temp(numer_id))
            .unwrap_or(*value.numer());
        let new_denom = ui
            .data_mut(|map| map.remove_temp(denom_id))
            .unwrap_or(*value.denom());

        let new_value = Ratio::new(new_numer, new_denom.max(1));
        if new_value != *value {
            value.clone_from(&new_value);
            return true;
        }
    }

    false
}

pub fn note_picker<T: StackType>(
    ui: &mut egui::Ui,
    tmp_temperaments: &mut [bool],
    tmp_correction: &mut Correction<T>,
    correction_system_index: usize,
    stack: &mut Stack<T>,
) {
    ui.vertical(|ui| {
        let mut target_changed = false;
        ui.horizontal(|ui| {
            for (i, c) in stack.target.iter_mut().enumerate() {
                ui.label(format!("{}:", T::intervals()[i].name));
                if ui.add(egui::DragValue::new(c)).changed() {
                    target_changed = true;
                }
            }
        });

        if target_changed {
            tmp_temperaments.iter_mut().for_each(|b| *b = false);
            tmp_correction.reset_to_zero();
            stack.make_pure();
        }

        ui.label("tempered with:");

        temperament_applier(None {}, ui, tmp_correction, correction_system_index, stack);
    });
}

/// returns true on change
pub fn temperament_applier<T: StackType>(
    reset_button_text: Option<&str>,
    ui: &mut egui::Ui,
    tmp_correction: &mut Correction<T>,
    correction_system_index: usize,
    stack: &mut Stack<T>,
) -> bool {
    let mut temperament_select_changed = false;
    let mut correction_changed = false;
    let mut made_pure = false;
    if reset_button_text.is_some() {
        ui.vertical_centered(|ui| {
            if ui
                .add_enabled(
                    !stack.is_target(),
                    egui::Button::new(reset_button_text.unwrap()),
                )
                .clicked()
            {
                tmp_correction.reset_to_zero();
                stack.make_pure();
                made_pure = true;
            }
        });
        ui.separator();
    }

    ui.horizontal(|ui| {
        if T::num_temperaments() > 0 {
            ui.vertical(|ui| {
                for (i, t) in T::temperaments().iter().enumerate() {
                    if ui.button(&t.name).clicked() {
                        stack.apply_temperament(i);
                        *tmp_correction = Correction::new(stack, correction_system_index);
                        temperament_select_changed = true;
                    }
                }
            });
            ui.separator();
        }

        tmp_correction.mutate(correction_system_index, |coeffs| {
            ui.vertical(|ui| {
                for (i, x) in coeffs.indexed_iter_mut() {
                    ui.horizontal(|ui| {
                        let name = &T::correction_systems()[correction_system_index].basis_names[i];
                        if rational_drag_value(ui, ui.id().with(name), x) {
                            correction_changed = true;
                        }
                        ui.label(name);
                    });
                }
            });
        });

        if correction_changed {
            stack.apply_correction(tmp_correction);
        }
    });
    temperament_select_changed | correction_changed | made_pure
}
