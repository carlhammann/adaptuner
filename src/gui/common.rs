use std::marker::PhantomData;

use eframe::egui;
use num_rational::Ratio;

use crate::{
    config::ExtractConfig,
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
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

pub struct OwningListEdit<X> {
    elems: Vec<X>,
    selected: Option<usize>,
}

pub struct RefListEdit<'a, X> {
    elems: &'a mut Vec<X>,
    selected: &'a mut Option<usize>,
}

pub struct ListEditOpts<X, M, H> {
    pub empty_allowed: bool,
    pub select_allowed: bool,
    pub no_selection_allowed: bool,
    pub delete_allowed: bool,
    pub reorder_allowed: bool,
    pub show_one: Box<dyn Fn(&mut egui::Ui, usize, &mut X, &mut H) -> Option<M>>,
    pub clone: Option<Box<dyn FnOnce(&mut egui::Ui, &[X], Option<usize>, &mut H) -> Option<usize>>>,
}

#[derive(PartialEq)]
pub enum ListEditResult<M> {
    Message(M),
    Action(ListAction),
    None,
}

pub trait ListEdit<X> {
    fn elems(&self) -> &[X];
    fn apply(&mut self, action: ListAction)
    where
        X: Clone;
    fn current_selected(&self) -> Option<&X>;
    fn current_selected_mut(&mut self) -> Option<&mut X>;
    fn show_as_list_picker(
        &mut self,
        ui: &mut egui::Ui,
        elem_name: impl Fn(&X) -> &str,
        elem_description: impl Fn(&X) -> Option<&str>,
    ) -> Option<(usize, &X)>;
    fn show<M, H>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M, H>,
        view_data: &mut H,
    ) -> ListEditResult<M>
    where
        X: Clone;
}

impl<'a, X> RefListEdit<'a, X> {
    pub fn new(elems: &'a mut Vec<X>, selected: &'a mut Option<usize>) -> Self {
        Self { elems, selected }
    }

    fn show_dont_handle<M, H>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M, H>,
        view_data: &mut H,
    ) -> ListEditResult<M> {
        let mut res = ListEditResult::None;
        let mut update_res = |new_res: ListEditResult<M>| match res {
            ListEditResult::None => {
                res = new_res;
            }
            _ => {}
        };
        let selected = *self.selected;
        egui::Grid::new(id_salt)
            .min_col_width(ui.style().spacing.interact_size.y)
            .with_row_color(move |i, style| {
                if Some(i) == selected {
                    Some(style.visuals.selection.bg_fill)
                } else if i % 2 == 0 {
                    Some(style.visuals.faint_bg_color)
                } else {
                    None {}
                }
            })
            .show(ui, |ui| {
                let n = self.elems.len();
                for (i, elem) in self.elems.iter_mut().enumerate() {
                    if opts.select_allowed {
                        let is_current = selected == Some(i);
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

                    if let Some(m) = (opts.show_one)(ui, i, elem, view_data) {
                        update_res(ListEditResult::Message(m));
                    }

                    if opts.reorder_allowed {
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
                    }

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
            if let Some(i) = f(ui, &self.elems, *self.selected, view_data) {
                update_res(ListEditResult::Action(ListAction::Clone(i)));
            }
        }

        res
    }
}

impl<'a, X> ListEdit<X> for RefListEdit<'a, X> {
    fn elems(&self) -> &[X] {
        &self.elems
    }

    fn apply(&mut self, action: ListAction)
    where
        X: Clone,
    {
        action.apply_to(|x| x.clone(), &mut self.elems, &mut self.selected);
    }

    fn current_selected(&self) -> Option<&X> {
        self.selected.map(|i| &self.elems[i])
    }

    fn current_selected_mut(&mut self) -> Option<&mut X> {
        self.selected.map(|i| &mut self.elems[i])
    }

    fn show_as_list_picker(
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

    fn show<M, H>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M, H>,
        view_data: &mut H,
    ) -> ListEditResult<M>
    where
        X: Clone,
    {
        let res = self.show_dont_handle(ui, id_salt, opts, view_data);
        if let ListEditResult::Action(action) = &res {
            action.apply_to(|x| x.clone(), &mut self.elems, &mut self.selected);
        }
        res
    }
}

impl<X> OwningListEdit<X> {
    pub fn new(elems: Vec<X>) -> Self {
        Self {
            elems,
            selected: None {},
        }
    }

    pub fn set_elems(&mut self, elems: &[X])
    where
        X: Clone,
    {
        self.elems = elems.into();
        self.selected = None {};
    }

    pub fn put_elems(&mut self, elems: Vec<X>) {
        self.elems = elems;
        self.selected = None {}
    }

    fn as_ref_list_edit<'a>(&'a mut self) -> RefListEdit<'a, X> {
        RefListEdit {
            elems: &mut self.elems,
            selected: &mut self.selected,
        }
    }
}

impl<X> ListEdit<X> for OwningListEdit<X> {
    fn elems(&self) -> &[X] {
        &self.elems
    }

    fn apply(&mut self, action: ListAction)
    where
        X: Clone,
    {
        action.apply_to(|x| x.clone(), &mut self.elems, &mut self.selected);
    }

    fn current_selected(&self) -> Option<&X> {
        self.selected.map(|i| &self.elems[i])
    }

    fn current_selected_mut(&mut self) -> Option<&mut X> {
        self.selected.map(|i| &mut self.elems[i])
    }

    fn show_as_list_picker(
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

    fn show<M, H>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        opts: ListEditOpts<X, M, H>,
        view_data: &mut H,
    ) -> ListEditResult<M>
    where
        X: Clone,
    {
        self.as_ref_list_edit().show(ui, id_salt, opts, view_data)
    }
}

pub struct SmallFloatingWindow {
    id: egui::Id,
    open: bool,
    bring_to_foreground: bool,
}

impl SmallFloatingWindow {
    pub fn new(id: egui::Id, open: bool) -> Self {
        Self {
            id,
            open,
            bring_to_foreground: open,
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

    pub fn is_open(&self) -> bool {
        self.open
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
                    nw: 2,
                    se: 0,
                    sw: 2,
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
                egui::Button::new("×").corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 2,
                    sw: 0,
                    se: 2,
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

pub struct CorrectionSystemChooser<T: IntervalBasis> {
    _phantom: PhantomData<T>,
    pub use_cent_values: bool,
    preference_order: OwningListEdit<usize>,
    id_salt: &'static str,
}

impl<T: IntervalBasis> ExtractConfig<bool> for CorrectionSystemChooser<T> {
    fn extract_config(&self) -> bool {
        self.use_cent_values
    }
}

impl<T: StackType> CorrectionSystemChooser<T> {
    pub fn new(id_salt: &'static str, use_cent_values: bool) -> Self {
        Self {
            _phantom: PhantomData,
            use_cent_values,
            preference_order: {
                let mut v = Vec::with_capacity(T::num_named_intervals());
                (0..T::num_named_intervals()).for_each(|i| v.push(i));
                OwningListEdit::new(v)
            },
            id_salt,
        }
    }

    pub fn preference_order(&self) -> &[usize] {
        &self.preference_order.elems
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.checkbox(&mut self.use_cent_values, "use cent values");
            ui.label("preference order for commas:");
            let _ = self.preference_order.show(
                ui,
                self.id_salt,
                ListEditOpts::<_, _, ()> {
                    empty_allowed: false,
                    select_allowed: false,
                    no_selection_allowed: false,
                    delete_allowed: false,
                    reorder_allowed: true,
                    show_one: Box::new(|ui, _, i, _| {
                        ui.label(format!(
                            "{} ('{}')",
                            T::named_intervals()[*i].name,
                            T::named_intervals()[*i].short_name
                        ));
                        None::<()> {}
                    }),
                    clone: None {},
                },
                &mut (),
            );
        });
    }
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
    stack: &mut Stack<T>,
    preference_order: &[usize],
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

        temperament_applier(None {}, ui, tmp_correction, stack, preference_order);
    });
}

/// returns true on change
pub fn temperament_applier<T: StackType>(
    reset_button_text: Option<&str>,
    ui: &mut egui::Ui,
    tmp_correction: &mut Correction<T>,
    stack: &mut Stack<T>,
    preference_order: &[usize],
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
                        if !tmp_correction.set_with(stack, preference_order) {
                            tmp_correction.reset_to_zero();
                        }
                        temperament_select_changed = true;
                    }
                }
            });
            ui.separator();
        }

        ui.vertical(|ui| {
            for (i, x) in tmp_correction.coeffs.indexed_iter_mut() {
                ui.horizontal(|ui| {
                    let name = &T::named_intervals()[i].name;
                    if rational_drag_value(ui, ui.id().with(name), x) {
                        correction_changed = true;
                    }
                    ui.label(name);
                });
            }
        });

        if correction_changed {
            stack.apply_correction(tmp_correction);
        }
    });
    temperament_select_changed | correction_changed | made_pure
}
