use eframe::egui;

use crate::{
    adaptors::{ViewKeyStates, ViewTunings},
    gui::{
        common::{show_list_edit, ListEditOpts, ListEditResult},
        r#trait::{ReceiveToUiRef, UiAdaptor},
    },
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{OctavePeriodicStackType, StackCoeff, StackType},
    },
    msg::ToUi,
    neighbourhood::{
        sounding_partial, sounding_periodic_partial, Neighbourhood, SomeNeighbourhood,
    },
    notename::{HasNoteNames, NoteNameStyle},
    strategy::harmony::chordlist::{blocks_from_current, keyshape::KeyShape, PatternConfig},
    util::list_action::ListAction,
};

#[derive(Clone)]
struct RememberedChord<T: StackType> {
    key_shape: KeyShape,
    neighbourhood: SomeNeighbourhood<T>,
    original_reference: Stack<T>,
}

pub struct ChordListEditor<T: StackType> {
    active_pattern: Option<usize>,
    request_recompute: bool,
    new_name: String,
    new_config: Option<RememberedChord<T>>,
    simple: bool,
    block_sizes: Vec<usize>,
    match_transpositions: bool,
    match_voicings: bool,
    allow_extra_high_notes: bool,
    tmp_stack: Stack<T>,
}

fn describe_key_shape<T: StackType + HasNoteNames>(
    ui: &mut egui::Ui,
    key_shape: &KeyShape,
    neighbourhood: &SomeNeighbourhood<T>,
    original_reference: &Stack<T>,
    use_cent_values: bool,
    tmp_stack: &mut Stack<T>,
) {
    match key_shape {
        KeyShape::ClassesRelative { .. } => {
            ui.label("match all voicings and all transpositions of:");
            neighbourhood.for_each_stack(|_offset, stack| {
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(&NoteNameStyle::Class, use_cent_values)
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
                    tmp_stack.corrected_notename(&NoteNameStyle::Class, use_cent_values)
                ));
            });
        }
        KeyShape::ExactFixed { .. } => {
            ui.label("match exactly this chord:");
            neighbourhood.for_each_stack(|_offset, relative_stack| {
                tmp_stack.clone_from(relative_stack);
                tmp_stack.scaled_add(1, original_reference);
                ui.label(format!(
                    "  {}",
                    tmp_stack.corrected_notename(&NoteNameStyle::Full, use_cent_values)
                ));
            });
        }
        KeyShape::ExactRelative { .. } => {
            ui.label("match all transpositions of this chord:");
            neighbourhood.for_each_stack(|_offset, stack| {
                ui.label(format!(
                    "  {}",
                    stack.corrected_notename(&NoteNameStyle::Full, use_cent_values)
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
                                tmp_stack
                                    .corrected_notename(&NoteNameStyle::Class, use_cent_values)
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
                                tmp_stack
                                    .corrected_notename(&NoteNameStyle::Class, use_cent_values)
                            ));
                        }
                    });
                }
            });
        }
    }
}

impl<T: OctavePeriodicStackType + HasNoteNames> ChordListEditor<T> {
    pub fn new() -> Self {
        Self {
            active_pattern: None {},
            simple: true,
            block_sizes: vec![1],
            match_voicings: true,
            match_transpositions: true,
            allow_extra_high_notes: true,
            request_recompute: true,
            new_name: String::with_capacity(16),
            new_config: None {},
            tmp_stack: Stack::new_zero(),
        }
    }

    fn recompute_simple<A: ViewKeyStates + ViewTunings<T>>(&mut self, adaptor: &A) {
        if let Some(lowest_sounding) = (0..128).position(|i| adaptor.key_state(i).is_sounding()) {
            self.new_config =
                Some(match (self.match_transpositions, self.match_voicings) {
                    (true, true) => RememberedChord {
                        key_shape: KeyShape::ClassesRelative {
                            classes: {
                                let mut active = [false; 12];
                                for i in 0..128 {
                                    if adaptor.key_state(i).is_sounding() {
                                        active[((i as isize - lowest_sounding as isize) % 12)
                                            as usize] = true;
                                    }
                                }
                                let mut classes = vec![];
                                for (i, b) in active.iter().enumerate() {
                                    if *b {
                                        classes.push(
                                            (i as isize - lowest_sounding as isize).rem_euclid(12)
                                                as u8,
                                        );
                                    }
                                }
                                classes
                            },
                        },
                        neighbourhood: SomeNeighbourhood::PeriodicPartial(
                            sounding_periodic_partial(adaptor, lowest_sounding),
                        ),
                        original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                    },
                    (false, true) => RememberedChord {
                        key_shape: KeyShape::ClassesFixed {
                            classes: {
                                let mut active = [false; 12];
                                for i in 0..128 {
                                    if adaptor.key_state(i).is_sounding() {
                                        active[i % 12] = true;
                                    }
                                }

                                let mut classes = vec![];
                                for (i, b) in active.iter().enumerate() {
                                    if *b {
                                        classes.push(i.rem_euclid(12) as u8);
                                    }
                                }
                                classes
                            },
                        },
                        neighbourhood: SomeNeighbourhood::PeriodicPartial(
                            sounding_periodic_partial(adaptor, lowest_sounding),
                        ),
                        original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                    },
                    (true, false) => RememberedChord {
                        key_shape: KeyShape::ExactRelative {
                            offsets: (0..128)
                                .filter(|i: &u8| adaptor.key_state(*i as usize).is_sounding())
                                .collect(),
                        },
                        neighbourhood: SomeNeighbourhood::Partial(sounding_partial(
                            adaptor,
                            lowest_sounding,
                        )),
                        original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                    },
                    (false, false) => RememberedChord {
                        key_shape: KeyShape::ExactFixed {
                            keys: (0..128)
                                .filter(|i: &u8| adaptor.key_state(*i as usize).is_sounding())
                                .collect(),
                        },
                        neighbourhood: SomeNeighbourhood::Partial(sounding_partial(
                            adaptor,
                            lowest_sounding,
                        )),
                        original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                    },
                });
        } else {
            self.new_config = None {};
        }
    }

    fn recompute_block<A: ViewKeyStates + ViewTunings<T>>(&mut self, adaptor: &A) {
        if let Some(lowest_sounding) = (0..128).position(|i| adaptor.key_state(i).is_sounding()) {
            self.new_config = Some(if self.match_transpositions {
                RememberedChord {
                    key_shape: KeyShape::BlockVoicingRelative {
                        blocks: blocks_from_current(&self.block_sizes, adaptor, lowest_sounding),
                    },
                    neighbourhood: SomeNeighbourhood::PeriodicPartial(sounding_periodic_partial(
                        adaptor,
                        lowest_sounding,
                    )),
                    original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                }
            } else {
                RememberedChord {
                    key_shape: KeyShape::BlockVoicingFixed {
                        blocks: blocks_from_current(&self.block_sizes, adaptor, lowest_sounding),
                    },
                    neighbourhood: SomeNeighbourhood::PeriodicPartial(sounding_periodic_partial(
                        adaptor,
                        lowest_sounding,
                    )),
                    original_reference: adaptor.tuning(lowest_sounding).stack.clone(),
                }
            });
        } else {
            self.new_config = None {};
        }
    }

    fn recompute_new_config<A: ViewKeyStates + ViewTunings<T>>(&mut self, adaptor: &A) {
        if self.simple {
            self.recompute_simple(adaptor);
        } else {
            self.recompute_block(adaptor);
        }
    }

    fn show_new_simple<A: ViewKeyStates + ViewTunings<T>>(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: &A,
    ) {
        let mut recompute = false;
        recompute |= ui
            .checkbox(&mut self.match_voicings, "match all voicings")
            .changed();
        recompute |= ui
            .checkbox(&mut self.match_transpositions, "match all transpositions")
            .changed();
        recompute |= ui
            .checkbox(
                &mut self.allow_extra_high_notes,
                "allow additional high notes, if no other entry fits perfectly",
            )
            .changed();

        if recompute {
            self.recompute_new_config(adaptor);
        }
    }

    fn show_new_block<A: ViewKeyStates + ViewTunings<T>>(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: &A,
    ) {
        let mut recompute = false;
        recompute |= ui
            .checkbox(&mut self.match_transpositions, "match all transpositions")
            .changed();
        recompute |= ui
            .checkbox(
                &mut self.allow_extra_high_notes,
                "allow additional high notes, if no other entry fits perfectly",
            )
            .changed();

        ui.separator();
        ui.label("block sizes (number of pitch classes in each block, lowest to highest):");
        let res = show_list_edit(
            ui,
            "block_size_editor",
            &mut self.block_sizes,
            None {},
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
            self.recompute_new_config(adaptor);
        }
    }
}

pub enum ChordListEditorResult {
    None,
    ToggleEnable,
    UpdateChord(usize),
    ListAction(ListAction),
    PushNewChord,
}

impl<T: OctavePeriodicStackType + HasNoteNames> ChordListEditor<T> {
    pub fn show_list(
        &mut self,
        ui: &mut egui::Ui,
        enable: &mut bool,
        patterns: &mut Vec<PatternConfig<T>>,
        use_cent_values: bool,
    ) -> ChordListEditorResult {
        let mut list_edit_res = ListEditResult::None;
        ui.vertical(|ui| {
            if !*enable {
                ui.disable();
            }

            list_edit_res = show_list_edit(
              ui,
              "chord_list_editor_list_edit",
              patterns,
              self.active_pattern,
              ListEditOpts {
                  empty_allowed: true,
                  select_allowed: false,
                  no_selection_allowed: true,
                  delete_allowed: true,
                  reorder_allowed: true,
                  show_one: Box::new(
                      |ui, i, pattern, (use_cent_values, tmp_stack): &mut (bool, &mut Stack<T>)| {
                          let mut updated_index = None {};
                          ui.horizontal(|ui| {
                              egui::ComboBox::from_id_salt(&pattern.key_shape)
                                  .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                  .selected_text(&pattern.name)
                                  .show_ui(ui, |ui| {
                                      ui.add(egui::TextEdit::singleline(&mut pattern.name).min_size(
                                          egui::vec2(
                                              ui.style().spacing.text_edit_width / 2.0,
                                              ui.style().spacing.interact_size.y,
                                          ),
                                      ));
                                      if ui
                                          .checkbox(
                                              &mut pattern.allow_extra_high_notes,
                                              "allow additional high notes, \
                                              if no other entry fits perfectly",
                                          )
                                          .clicked()
                                      {
                                          pattern.allow_extra_high_notes = true;
                                          updated_index = Some(i);
                                      }
                                      describe_key_shape(
                                          ui,
                                          &pattern.key_shape,
                                          &pattern.neighbourhood,
                                          &pattern.original_reference,
                                          *use_cent_values,
                                          *tmp_stack,
                                      );
                                  });
                          });
                          updated_index
                      },
                  ),
                  clone: None {},
              },
              &mut (use_cent_values, &mut self.tmp_stack),
          );
        });
        match list_edit_res {
            ListEditResult::None => ChordListEditorResult::None,
            ListEditResult::Action(action) => {
                action.apply_to_no_select(patterns, |x| x.clone());
                ChordListEditorResult::ListAction(action)
            }
            ListEditResult::Message(i) => ChordListEditorResult::UpdateChord(i),
        }
    }

    pub fn show_new_chord<A: ViewKeyStates + ViewTunings<T>>(
        &mut self,
        ui: &mut egui::Ui,
        patterns: &mut Vec<PatternConfig<T>>,
        adaptor: &A,
    ) -> ChordListEditorResult {
        ui.label("Add a new entry capturing the currently sounding chord");

        ui.horizontal(|ui| {
            ui.label("name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_name).min_size(egui::vec2(
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
            self.show_new_simple(ui, adaptor);
        } else {
            self.show_new_block(ui, adaptor);
        }

        if ui
            .vertical_centered(|ui| {
                ui.add_enabled(
                    self.new_config.is_some() && self.active_pattern.is_none(),
                    egui::Button::new("add"),
                )
                .clicked()
            })
            .inner
        {
            let RememberedChord {
                key_shape,
                neighbourhood,
                original_reference,
            } = self.new_config.take().unwrap();
            patterns.push(PatternConfig {
                name: if self.new_name.is_empty() {
                    String::from("unnamed")
                } else {
                    self.new_name.clone()
                },
                key_shape,
                neighbourhood,
                allow_extra_high_notes: self.allow_extra_high_notes,
                original_reference,
            });
            self.new_name.clear();
            ChordListEditorResult::PushNewChord
        } else {
            ChordListEditorResult::None
        }
    }

    pub fn show<A: ViewKeyStates + ViewTunings<T>>(
        &mut self,
        ui: &mut egui::Ui,
        enable: &mut bool,
        patterns: &mut Vec<PatternConfig<T>>,
        adaptor: &A,
        use_cent_values: bool,
    ) -> ChordListEditorResult {
        if self.request_recompute {
            self.recompute_new_config(adaptor);
            self.request_recompute = false;
        }

        let mut res = ChordListEditorResult::None;

        let mut update_res = |new| match res {
            ChordListEditorResult::None => res = new,
            _ => {}
        };

        ui.collapsing("chords", |ui| {
            ui.vertical_centered(|ui| {
                if ui
                    .button(if *enable { "disable" } else { "enable" })
                    .clicked()
                {
                    *enable = !*enable;
                    update_res(ChordListEditorResult::ToggleEnable);
                }
            });

            ui.separator();

            update_res(self.show_list(ui, enable, patterns, use_cent_values));

            ui.separator();

            update_res(self.show_new_chord(ui, patterns, adaptor));

            if let (
                None {},
                Some(RememberedChord {
                    key_shape,
                    neighbourhood,
                    original_reference,
                }),
            ) = (self.active_pattern, &self.new_config)
            {
                describe_key_shape(
                    ui,
                    key_shape,
                    neighbourhood,
                    original_reference,
                    use_cent_values,
                    &mut self.tmp_stack,
                );
            }
        });

        res
    }
}

impl<T: StackType, A: UiAdaptor<T>> ReceiveToUiRef<T, A> for ChordListEditor<T> {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, _adaptor: &A) {
        match msg {
            ToUi::CurrentHarmony { pattern_index, .. } => {
                self.active_pattern = *pattern_index;
            }

            ToUi::NoteOn { .. }
            | ToUi::NoteOff { .. }
            | ToUi::PedalHold { .. }
            | ToUi::Retune { .. } => {
                self.request_recompute = true;
            }

            _ => {}
        }
    }
}
