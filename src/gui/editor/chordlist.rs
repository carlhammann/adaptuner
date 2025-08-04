use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    config::NamedPatternConfig,
    gui::{
        common::{CorrectionSystemChooser, ListEdit, ListEditOpts, RefListEdit},
        toplevel::KeysAndTunings,
    },
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::StackType,
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::{Neighbourhood, SomeNeighbourhood},
    notename::{HasNoteNames, NoteNameStyle},
    strategy::twostep::harmony::chordlist::{keyshape::KeyShape, PatternConfig},
};

pub struct ChordListEditor<T: StackType> {
    enabled: bool,
    active_pattern: Option<usize>,
    recompute: bool,
    new_name: String,
    new_config: Option<(PatternConfig<T>, Stack<T>)>,
    match_transpositions: bool,
    match_voicings: bool,
    allow_extra_high_notes: bool,
    correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
}

fn describe_pattern<T: StackType + HasNoteNames>(
    ui: &mut egui::Ui,
    key_shape: &KeyShape,
    neighbourhood: &SomeNeighbourhood<T>,
    original_reference: &Stack<T>,
    correction_system_chooser: &CorrectionSystemChooser<T>,
) {
    match key_shape {
        KeyShape::ClassesRelative { .. } => {
            ui.label("match all voicings and all transpositions of:");
            neighbourhood.for_each_stack(|_offset, stack| {
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(
                        &NoteNameStyle::Class,
                        correction_system_chooser.preference_order(),
                        correction_system_chooser.use_cent_values
                    )
                ));
            });
        }
        KeyShape::ClassesFixed { .. } => {
            ui.label("match all voicings of:");
            let mut stack = Stack::new_zero();
            neighbourhood.for_each_stack(|_offset, relative_stack| {
                stack.clone_from(relative_stack);
                stack.scaled_add(1, original_reference);
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(
                        &NoteNameStyle::Class,
                        correction_system_chooser.preference_order(),
                        correction_system_chooser.use_cent_values
                    )
                ));
            });
        }
        KeyShape::ExactFixed { .. } => {
            ui.label("match exactly this chord:");
            neighbourhood.for_each_stack(|_offset, stack| {
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(
                        &NoteNameStyle::Full,
                        correction_system_chooser.preference_order(),
                        correction_system_chooser.use_cent_values
                    )
                ));
            });
        }
        KeyShape::ExactRelative { .. } => {
            ui.label("match any transposition of this chord:");
            let mut stack = Stack::new_zero();
            neighbourhood.for_each_stack(|_offset, relative_stack| {
                stack.clone_from(relative_stack);
                stack.scaled_add(1, original_reference);
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(
                        &NoteNameStyle::Full,
                        correction_system_chooser.preference_order(),
                        correction_system_chooser.use_cent_values
                    )
                ));
            });
        }
        _ => todo!(),
    }
}

impl<T: StackType + HasNoteNames> ChordListEditor<T> {
    pub fn new(correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>) -> Self {
        Self {
            enabled: true,
            active_pattern: None {},
            match_voicings: true,
            match_transpositions: true,
            allow_extra_high_notes: true,
            correction_system_chooser,
            recompute: true,
            new_name: String::with_capacity(16),
            new_config: None {},
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        patterns: &mut Vec<NamedPatternConfig<T>>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        ui.vertical_centered(|ui| {
            if ui
                .button(if self.enabled { "disable" } else { "enable" })
                .clicked()
            {
                self.enabled = !self.enabled;
                let _ = forward.send(FromUi::EnableChordList {
                    enable: self.enabled,
                    time: Instant::now(),
                });
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            if !self.enabled {
                ui.disable();
            }

            // the self.active_pattern won't be changed, because select_allowed = false
            let res = RefListEdit::new(patterns, &mut self.active_pattern).show(
                ui,
                "chord_list_editor_list_edit",
                ListEditOpts {
                    empty_allowed: true,
                    select_allowed: false,
                    no_selection_allowed: true,
                    delete_allowed: true,
                    show_one: Box::new(
                        |ui, i, pattern, correction_system_chooser: &CorrectionSystemChooser<T>| {
                            let mut msg = None {};
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(&pattern.key_shape)
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                    .selected_text(&pattern.name)
                                    .show_ui(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut pattern.name).min_size(
                                                vec2(
                                                    ui.style().spacing.text_edit_width / 2.0,
                                                    ui.style().spacing.interact_size.y,
                                                ),
                                            ),
                                        );
                                        if ui
                                            .checkbox(
                                                &mut pattern.allow_extra_high_notes,
                                                "allow additional high notes, \
                                            if no other entry fits perfectly",
                                            )
                                            .clicked()
                                        {
                                            msg = Some(FromUi::AllowExtraHighNotes {
                                                pattern_index: i,
                                                allow: pattern.allow_extra_high_notes,
                                                time: Instant::now(),
                                            });
                                        }
                                        describe_pattern(
                                            ui,
                                            &pattern.key_shape,
                                            &pattern.neighbourhood,
                                            &pattern.original_reference,
                                            correction_system_chooser,
                                        );
                                    });
                            });
                            msg
                        },
                    ),
                    clone: None {},
                },
                &*self.correction_system_chooser.borrow(),
            );

            match res {
                crate::gui::common::ListEditResult::Message(msg) => {
                    let _ = forward.send(msg);
                }
                crate::gui::common::ListEditResult::Action(action) => {
                    let _ = forward.send(FromUi::ChordListAction {
                        action,
                        time: Instant::now(),
                    });
                }
                crate::gui::common::ListEditResult::None => {}
            }
        });

        ui.separator();

        ui.label("Add a new entry capturing the currently sounding chord");

        ui.horizontal(|ui| {
            ui.label("name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_name).min_size(vec2(
                    ui.style().spacing.text_edit_width / 2.0,
                    ui.style().spacing.interact_size.y,
                )),
            );
        });

        // don't use short-circuiting here, we want to show all checkboxes always.
        if ui
            .checkbox(&mut self.match_voicings, "match all voicings")
            .changed()
            | ui.checkbox(&mut self.match_transpositions, "match all transpositions")
                .changed()
            | ui.checkbox(
                &mut self.allow_extra_high_notes,
                "allow additional high notes, if no other entry fits perfectly",
            )
            .changed()
        {
            self.recompute = true;
        }

        if self.recompute {
            self.recompute = false;
            if let Some(lowest_sounding) = state.active_notes.iter().position(|k| k.is_sounding()) {
                self.new_config = match (self.match_transpositions, self.match_voicings) {
                    (true, true) => Some((
                        PatternConfig::classes_relative_from_current(
                            &state.active_notes,
                            &state.tunings,
                            lowest_sounding,
                            self.allow_extra_high_notes,
                        ),
                        state.tunings[lowest_sounding].clone(),
                    )),
                    (false, true) => Some((
                        PatternConfig::classes_fixed_from_current(
                            &state.active_notes,
                            &state.tunings,
                            lowest_sounding,
                            self.allow_extra_high_notes,
                        ),
                        state.tunings[lowest_sounding].clone(),
                    )),
                    (false, false) => Some((
                        PatternConfig::exact_fixed_from_current(
                            &state.active_notes,
                            &state.tunings,
                            self.allow_extra_high_notes,
                        ),
                        state.tunings[lowest_sounding].clone(),
                    )),
                    (true, false) => Some((
                        PatternConfig::exact_relative_from_current(
                            &state.active_notes,
                            &state.tunings,
                            lowest_sounding,
                            self.allow_extra_high_notes,
                        ),
                        state.tunings[lowest_sounding].clone(),
                    )),
                };
            } else {
                self.new_config = None {};
            }
        }

        if let (None {}, Some((pattern, original_reference))) =
            (self.active_pattern, &self.new_config)
        {
            describe_pattern(
                ui,
                &pattern.key_shape,
                &pattern.neighbourhood,
                original_reference,
                &*self.correction_system_chooser.borrow(),
            );
        }

        ui.vertical_centered(|ui| {
            if ui
                .add_enabled(
                    self.new_config.is_some() && self.active_pattern.is_none(),
                    egui::Button::new("add"),
                )
                .clicked()
            {
                let (conf, original_reference) = self.new_config.as_ref().unwrap().clone();
                let _ = forward.send(FromUi::PushNewChord {
                    pattern: conf.clone(),
                    time: Instant::now(),
                });
                patterns.push(NamedPatternConfig {
                    name: if self.new_name.is_empty() {
                        String::from("unnamed")
                    } else {
                        self.new_name.clone()
                    },
                    key_shape: conf.key_shape,
                    neighbourhood: conf.neighbourhood,
                    allow_extra_high_notes: conf.allow_extra_high_notes,
                    original_reference,
                });
                self.new_name.clear();
            }
        });
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ChordListEditor<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentHarmony { pattern_index, .. } => {
                self.active_pattern.clone_from(pattern_index);
            }

            ToUi::NoteOn { .. }
            | ToUi::TunedNoteOn { .. }
            | ToUi::NoteOff { .. }
            | ToUi::PedalHold { .. } => {
                self.recompute = true;
            }

            _ => {}
        }
    }
}
