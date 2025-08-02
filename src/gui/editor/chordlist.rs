use std::{cell::RefCell, marker::PhantomData, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, vec2};

use crate::{
    config::NamedPatternConfig,
    gui::common::{CorrectionSystemChooser, ListEdit, ListEditOpts, RefListEdit},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::StackType,
    },
    keystate::KeyState,
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::Neighbourhood,
    notename::{HasNoteNames, NoteNameStyle},
    strategy::twostep::harmony::chordlist::{keyshape::KeyShape, PatternConfig},
};

pub struct ChordListEditor<T: StackType> {
    _phantom: PhantomData<T>,
    active_pattern: Option<usize>,
    new_pattern_name: String,
    match_transpositions: bool,
    match_voicings: bool,
    allow_extra_high_notes: bool,
    correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
}

impl<T: StackType + HasNoteNames> ChordListEditor<T> {
    pub fn new(correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>) -> Self {
        Self {
            _phantom: PhantomData,
            active_pattern: None {},
            new_pattern_name: String::with_capacity(32),
            match_voicings: true,
            match_transpositions: true,
            allow_extra_high_notes: true,
            correction_system_chooser,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
        patterns: &mut Vec<NamedPatternConfig<T>>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
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
                                    ui.add(egui::TextEdit::singleline(&mut pattern.name).min_size(
                                        vec2(
                                            ui.style().spacing.text_edit_width / 2.0,
                                            ui.style().spacing.interact_size.y,
                                        ),
                                    ));
                                    if ui
                                        .checkbox(
                                            &mut pattern.allow_extra_high_notes,
                                            "allow additional \
                                        high notes, if no other entry fits perfectly",
                                        )
                                        .clicked()
                                    {
                                        msg = Some(FromUi::AllowExtraHighNotes {
                                            pattern_index: i,
                                            allow: pattern.allow_extra_high_notes,
                                            time: Instant::now(),
                                        });
                                    }
                                    match &pattern.key_shape {
                                        KeyShape::ClassesRelative { .. } => {
                                            ui.label(
                                                "matches all voicings and all transpositions of:",
                                            );
                                            pattern.neighbourhood.for_each_stack(
                                                |_offset, stack| {
                                                    ui.label(format!(
                                                        "  {}",
                                                        stack.corrected_notename(
                                                            &NoteNameStyle::Class,
                                                            correction_system_chooser
                                                                .preference_order(),
                                                            correction_system_chooser
                                                                .use_cent_values
                                                        )
                                                    ));
                                                },
                                            );
                                        }
                                        KeyShape::ClassesFixed { .. } => {
                                            ui.label("matches all voicings of:");
                                            let mut stack = Stack::new_zero();
                                            pattern.neighbourhood.for_each_stack(
                                                |_offset, relative_stack| {
                                                    stack.clone_from(relative_stack);
                                                    stack
                                                        .scaled_add(1, &pattern.original_reference);
                                                    ui.label(format!(
                                                        "  {}",
                                                        stack.corrected_notename(
                                                            &NoteNameStyle::Class,
                                                            correction_system_chooser
                                                                .preference_order(),
                                                            correction_system_chooser
                                                                .use_cent_values
                                                        )
                                                    ));
                                                },
                                            );
                                        }
                                        _ => todo!(),
                                    }
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

        ui.separator();

        ui.label("Add a new entry capturing the currently sounding chord");

        ui.horizontal(|ui| {
            ui.label("name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_pattern_name).min_size(vec2(
                    ui.style().spacing.text_edit_width / 2.0,
                    ui.style().spacing.interact_size.y,
                )),
            );
        });

        ui.checkbox(
            &mut self.match_voicings,
            "match all voicings",
        );
        ui.checkbox(&mut self.match_transpositions, "match all transpositions");
        ui.checkbox(
            &mut self.allow_extra_high_notes,
            "allow additional high notes, if no other entry fits perfectly",
        );

        let lowest_sounding: Option<usize> = keys.iter().position(|k| k.is_sounding());
        let mut pattern_config_and_original_reference = None {};
        ui.vertical_centered(|ui| {
            if ui
                .add_enabled(
                    lowest_sounding.is_some() && !self.new_pattern_name.is_empty(),
                    egui::Button::new("add"),
                )
                .clicked()
            {
                match (self.match_transpositions, self.match_voicings) {
                    (true, true) => {
                        pattern_config_and_original_reference = Some((
                            PatternConfig::classes_relative_from_current(
                                keys,
                                tunings,
                                lowest_sounding.unwrap(),
                                self.allow_extra_high_notes,
                            ),
                            tunings[lowest_sounding.unwrap()].clone(),
                        ))
                    }
                    (false, true) => {
                        pattern_config_and_original_reference = Some((
                            PatternConfig::classes_fixed_from_current(
                                keys,
                                tunings,
                                lowest_sounding.unwrap(),
                                self.allow_extra_high_notes,
                            ),
                            tunings[lowest_sounding.unwrap()].clone(),
                        ))
                    }
                    _ => todo!(),
                }
            }
        });
        if let Some((pattern_config, original_reference)) = pattern_config_and_original_reference {
            let _ = forward.send(FromUi::PushNewChord {
                pattern: pattern_config.clone(),
                time: Instant::now(),
            });
            patterns.push(NamedPatternConfig {
                name: self.new_pattern_name.clone(),
                key_shape: pattern_config.key_shape,
                neighbourhood: pattern_config.neighbourhood,
                allow_extra_high_notes: pattern_config.allow_extra_high_notes,
                original_reference,
            });
            self.new_pattern_name.clear();
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ChordListEditor<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentHarmony {
                pattern_index,
                ..
                // reference,
            } => {
                self.active_pattern.clone_from(pattern_index);
                // self.reference_tuning.clone_from(reference);
            }
            _ => {}
        }
    }
}
