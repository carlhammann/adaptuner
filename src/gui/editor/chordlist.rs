use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    config::NamedPatternConfig,
    gui::{
        common::{CorrectionSystemChooser, ListEdit, ListEditOpts, ListEditResult, RefListEdit},
        toplevel::KeysAndTunings,
    },
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::{Neighbourhood, SomeNeighbourhood},
    notename::{HasNoteNames, NoteNameStyle},
    strategy::twostep::harmony::chordlist::{keyshape::KeyShape, PatternConfig},
};

pub struct ChordListEditor<T: StackType> {
    enabled: bool,
    active_pattern: Option<usize>,
    request_recompute: bool,
    new_name: String,
    new_config: Option<(PatternConfig<T>, Stack<T>)>,
    simple: bool,
    block_sizes: Vec<usize>,
    match_transpositions: bool,
    match_voicings: bool,
    allow_extra_high_notes: bool,
    correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    tmp_stack: Stack<T>,
}

fn describe_pattern<T: StackType + HasNoteNames>(
    ui: &mut egui::Ui,
    key_shape: &KeyShape,
    neighbourhood: &SomeNeighbourhood<T>,
    original_reference: &Stack<T>,
    correction_system_chooser: &CorrectionSystemChooser<T>,
    tmp_stack: &mut Stack<T>,
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
            neighbourhood.for_each_stack(|_offset, relative_stack| {
                tmp_stack.clone_from(relative_stack);
                tmp_stack.scaled_add(1, original_reference);
                ui.label(format!(
                    "  {}",
                    tmp_stack.corrected_notename(
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
            ui.label("match all transpositions of this chord:");
            neighbourhood.for_each_stack(|_offset, relative_stack| {
                tmp_stack.clone_from(relative_stack);
                tmp_stack.scaled_add(1, original_reference);
                ui.label(format!(
                    "  {}",
                    tmp_stack.corrected_notename(
                        &NoteNameStyle::Full,
                        correction_system_chooser.preference_order(),
                        correction_system_chooser.use_cent_values
                    )
                ));
            });
        }
        KeyShape::BlockVoicingFixed { blocks, .. } => {
            ui.label("match all block voicings of:");
            ui.horizontal(|ui| {
                for block in blocks {
                    ui.vertical(|ui| {
                        for offset in block {
                            // this will always succeed, because `neighbourhood` contains all pitch
                            // classes in the `blocks`
                            let _ = neighbourhood
                                .try_write_relative_stack(tmp_stack, *offset as StackCoeff);
                            tmp_stack.scaled_add(1, original_reference);

                            ui.label(format!(
                                "  {}",
                                tmp_stack.corrected_notename(
                                    &NoteNameStyle::Class,
                                    correction_system_chooser.preference_order(),
                                    correction_system_chooser.use_cent_values
                                )
                            ));
                        }
                    });
                }
            });
        }
        KeyShape::BlockVoicingRelative { blocks } => {
            ui.label("match all transpositions of all block voicings of:");
            ui.horizontal(|ui| {
                for block in blocks {
                    ui.vertical(|ui| {
                        for offset in block {
                            // this will always succeed, because `neighbourhood` contains all pitch
                            // classes in the `blocks`
                            let _ = neighbourhood
                                .try_write_relative_stack(tmp_stack, *offset as StackCoeff);

                            ui.label(format!(
                                "  {}",
                                tmp_stack.corrected_notename(
                                    &NoteNameStyle::Class,
                                    correction_system_chooser.preference_order(),
                                    correction_system_chooser.use_cent_values
                                )
                            ));
                        }
                    });
                }
            });
        }
    }
}

impl<T: StackType + HasNoteNames> ChordListEditor<T> {
    pub fn new(correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>) -> Self {
        Self {
            enabled: true,
            active_pattern: None {},
            simple: true,
            block_sizes: vec![1],
            match_voicings: true,
            match_transpositions: true,
            allow_extra_high_notes: true,
            correction_system_chooser,
            request_recompute: true,
            new_name: String::with_capacity(16),
            new_config: None {},
            tmp_stack: Stack::new_zero(),
        }
    }

    fn recompute_simple(&mut self, state: &KeysAndTunings<T>) {
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

    fn recompute_block(&mut self, state: &KeysAndTunings<T>) {
        if let Some(lowest_sounding) = state.active_notes.iter().position(|k| k.is_sounding()) {
            self.new_config = if self.match_transpositions {
                Some((
                    PatternConfig::block_voicing_relative_from_current(
                        &self.block_sizes,
                        &state.active_notes,
                        &state.tunings,
                        lowest_sounding,
                        self.allow_extra_high_notes,
                    ),
                    state.tunings[lowest_sounding].clone(),
                ))
            } else {
                Some((
                    PatternConfig::block_voicing_fixed_from_current(
                        &self.block_sizes,
                        &state.active_notes,
                        &state.tunings,
                        lowest_sounding,
                        self.allow_extra_high_notes,
                    ),
                    state.tunings[lowest_sounding].clone(),
                ))
            }
        } else {
            self.new_config = None {};
        }
    }

    fn recompute_new_config(&mut self, state: &KeysAndTunings<T>) {
        if self.simple {
            self.recompute_simple(state);
        } else {
            self.recompute_block(state);
        }
    }

    fn show_new_simple(&mut self, ui: &mut egui::Ui, state: &KeysAndTunings<T>) {
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
            self.recompute_new_config(state);
        }
    }

    fn show_new_block(&mut self, ui: &mut egui::Ui, state: &KeysAndTunings<T>) {
        let mut recompute = false;
        // don't use short-circuiting here, we want to show all checkboxes always.
        if ui
            .checkbox(&mut self.match_transpositions, "match all transpositions")
            .changed()
            | ui.checkbox(
                &mut self.allow_extra_high_notes,
                "allow additional high notes, if no other entry fits perfectly",
            )
            .changed()
        {
            recompute = true;
        }

        ui.separator();
        ui.label("block sizes (number of pitch classes in each block, lowest to highest):");
        let mut dummy = None {};
        let res = RefListEdit::new(&mut self.block_sizes, &mut dummy).show(
            ui,
            "block_size_editor",
            ListEditOpts {
                empty_allowed: false,
                select_allowed: false,
                no_selection_allowed: false,
                delete_allowed: true,
                reorder_allowed: false,
                show_one: Box::new(|ui, _, block_size, _| {
                    if ui
                        .add(egui::DragValue::new(block_size).range(1..=128))
                        .changed()
                    {
                        Some(())
                    } else {
                        None {}
                    }
                }),
                clone: Some(Box::new(|ui, elems, _, _| {
                    if ui.button("add a block").clicked() {
                        Some(elems.len() - 1)
                    } else {
                        None {}
                    }
                })),
            },
            &mut (),
        );
        if res != ListEditResult::None {
            recompute = true;
        }

        ui.separator();

        if recompute {
            self.recompute_new_config(state);
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        patterns: &mut Vec<NamedPatternConfig<T>>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        if self.request_recompute {
            self.recompute_new_config(state);
            self.request_recompute = false;
        }

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
                    reorder_allowed: true,
                    show_one: Box::new(
                        |ui,
                         i,
                         pattern,
                         (correction_system_chooser, tmp_stack): &mut (
                            &CorrectionSystemChooser<T>,
                            &mut Stack<T>,
                        )| {
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
                                            tmp_stack,
                                        );
                                    });
                            });
                            msg
                        },
                    ),
                    clone: None {},
                },
                &mut (
                    &*self.correction_system_chooser.borrow(),
                    &mut self.tmp_stack,
                ),
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

        ui.horizontal(|ui| {
            if ui
                .selectable_value(&mut self.simple, true, "simple chord or voicing")
                .clicked()
            {
                self.request_recompute = true;
            }
            if ui
                .selectable_value(&mut self.simple, false, "block voicing")
                .clicked()
            {
                self.request_recompute = true;
            }
        });

        if self.simple {
            self.show_new_simple(ui, state);
        } else {
            self.show_new_block(ui, state);
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

        if let (None {}, Some((pattern, original_reference))) =
            (self.active_pattern, &self.new_config)
        {
            describe_pattern(
                ui,
                &pattern.key_shape,
                &pattern.neighbourhood,
                original_reference,
                &*self.correction_system_chooser.borrow(),
                &mut self.tmp_stack,
            );
        }
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
                self.request_recompute = true;
            }

            ToUi::EnableChordList { enable } => self.enabled = *enable,

            _ => {}
        }
    }
}
