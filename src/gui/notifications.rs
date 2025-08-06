use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

use eframe::egui;

use crate::{
    config::{HarmonyStrategyNames, MelodyStrategyNames, StrategyNames},
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{HasNoteNames, NoteNameStyle},
};

use super::{common::CorrectionSystemChooser, toplevel::KeysAndTunings};

pub struct Notifications<T: StackType> {
    chord: (Option<(usize, Stack<T>)>, Instant),
    reference: (Option<Stack<T>>, Instant),
    neighbourhood_index: (Option<usize>, Instant),
    enable_chord_list: (Option<bool>, Instant),
    detuned_notes: VecDeque<(u8, Semitones, Semitones, &'static str, Instant)>,
    correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
}

impl<T: StackType + HasNoteNames> Notifications<T> {
    pub fn new(correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>) -> Self {
        Self {
            chord: (None {}, Instant::now()),
            reference: (None {}, Instant::now()),
            neighbourhood_index: (None {}, Instant::now()),
            enable_chord_list: (None {}, Instant::now()),
            detuned_notes: VecDeque::new(),
            correction_system_chooser,
        }
    }

    pub fn clear_old(&mut self, time: Instant) {
        // if let (Some(_), chord_time) = self.chord {
        //     if time.duration_since(chord_time) > Duration::from_secs(1) {
        //         self.chord = (None {}, time);
        //     }
        // }

        if let (Some(_), old) = self.reference {
            if time.duration_since(old) > Duration::from_secs(1) {
                self.reference = (None {}, time);
            }
        }

        if let (Some(_), old) = self.neighbourhood_index {
            if time.duration_since(old) > Duration::from_secs(1) {
                self.neighbourhood_index = (None {}, time);
            }
        }

        if let (Some(_), old) = self.enable_chord_list {
            if time.duration_since(old) > Duration::from_secs(1) {
                self.enable_chord_list = (None {}, time);
            }
        }

        loop {
            if let Some((_, _, _, _, old)) = self.detuned_notes.front() {
                if time.duration_since(*old) > Duration::from_secs(1) {
                    let _ = self.detuned_notes.pop_front();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn is_nonempty(&self) -> bool {
        self.chord.0.is_some()
            || self.reference.0.is_some()
            || self.neighbourhood_index.0.is_some()
            || self.enable_chord_list.0.is_some()
            || !self.detuned_notes.is_empty()
    }

    pub fn show(
        &self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        info: Option<&StrategyNames<T>>,
    ) {
        if let (Some(neighbourhood_index), _) = self.neighbourhood_index {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("switched to neighbourhood ");
                ui.strong(match info {
                    Some(StrategyNames::TwoStep {
                        melody:
                            MelodyStrategyNames::Neighbourhoods {
                                neighbourhood_names,
                                ..
                            },
                        ..
                    }) => &neighbourhood_names[neighbourhood_index],
                    Some(StrategyNames::StaticTuning {
                        neighbourhood_names,
                        ..
                    }) => &neighbourhood_names[neighbourhood_index],
                    _ => "<no name>",
                });
            });
        }

        if let (Some(enabled), _) = self.enable_chord_list {
            if enabled {
                ui.label("chord matching enabled");
            } else {
                ui.label("chord matching disabled");
            }
        }

        if let (Some((pattern_index, reference)), _) = &self.chord {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.strong(match info {
                    Some(StrategyNames::TwoStep {
                        harmony: HarmonyStrategyNames::ChordList { patterns },
                        ..
                    }) => &patterns[*pattern_index].name,
                    _ => "<no name>",
                });
                ui.label(" on ");
                ui.strong(reference.corrected_notename(
                    &NoteNameStyle::Full,
                    self.correction_system_chooser.borrow().preference_order(),
                    self.correction_system_chooser.borrow().use_cent_values,
                ));
            });
        }

        if let (Some(reference), _) = &self.reference {
            ui.horizontal(|ui| {
                ui.label("reference: ");
                ui.strong(reference.corrected_notename(
                    &NoteNameStyle::Full,
                    self.correction_system_chooser.borrow().preference_order(),
                    self.correction_system_chooser.borrow().use_cent_values,
                ));
            });
        }

        for (note, should_be, actual, explanation, _) in &self.detuned_notes {
            ui.label(format!(
                "note {} not tuned correctly: should be \
                {should_be:.02}, but is {actual:.02}: {explanation}",
                state.tunings[*note as usize].corrected_notename(
                    &NoteNameStyle::Full,
                    self.correction_system_chooser.borrow().preference_order(),
                    self.correction_system_chooser.borrow().use_cent_values,
                ),
            ));
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for Notifications<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::SetReference { stack } => {
                self.reference = (Some(stack.clone()), Instant::now());
            }
            ToUi::CurrentNeighbourhoodIndex { index } => {
                self.neighbourhood_index = (Some(*index), Instant::now());
            }
            ToUi::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => {
                self.detuned_notes.push_back((
                    *note,
                    *should_be,
                    *actual,
                    explanation,
                    Instant::now(),
                ));
            }
            ToUi::CurrentHarmony {
                pattern_index,
                reference,
            } => {
                if let (Some(i), Some(r)) = (pattern_index, reference) {
                    self.chord = (Some((*i, r.clone())), Instant::now());
                } else {
                    self.chord = (None, Instant::now());
                }
            }
            ToUi::EnableChordList { enable } => {
                self.enable_chord_list = (Some(*enable), Instant::now());
            }

            ToUi::CurrentStrategyIndex(_) => {}
            ToUi::Notify { .. } => {} // this will only contain MIDI parse errors (which shouldn't happen?)
            _ => {}
        }
    }
}
