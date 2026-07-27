use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use eframe::egui;

use crate::{
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    gui::r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg::ToUi,
    notename::{HasNoteNames, NoteNameStyle},
    strategy::{
        harmony::chordlist::ChordListConfig,
        melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
    util::ordered_locks::{OrderedLocks, Zero},
};

pub struct Notifications<T: StackType> {
    chord: (Option<(usize, Stack<T>)>, Instant),
    reference: (bool, Instant),
    neighbourhood_index: (Option<usize>, Instant),
    enable_reanchor: (Option<bool>, Instant),
    detuned_notes: VecDeque<(u8, Semitones, Semitones, &'static str, Instant)>,
    cleanup_time: Duration,
}

impl<T: StackType + HasNoteNames> Notifications<T> {
    pub fn new() -> Self {
        Self {
            chord: (None {}, Instant::now()),
            reference: (false, Instant::now()),
            neighbourhood_index: (None {}, Instant::now()),
            enable_reanchor: (None {}, Instant::now()),
            detuned_notes: VecDeque::new(),
            cleanup_time: Duration::from_secs(2),
        }
    }

    pub fn clear_old(&mut self, time: Instant) {
        // if let (Some(_), chord_time) = self.chord {
        //     if time.duration_since(chord_time) > self.cleanup_time {
        //         self.chord = (None {}, time);
        //     }
        // }

        if time.duration_since(self.reference.1) > self.cleanup_time {
            self.reference.0 = false;
        }

        if let (Some(_), old) = self.neighbourhood_index {
            if time.duration_since(old) > self.cleanup_time {
                self.neighbourhood_index = (None {}, time);
            }
        }

        if let (Some(_), old) = self.enable_reanchor {
            if time.duration_since(old) > self.cleanup_time {
                self.enable_reanchor = (None {}, time);
            }
        }

        loop {
            if let Some((_, _, _, _, old)) = self.detuned_notes.front() {
                if time.duration_since(*old) > self.cleanup_time {
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
            || self.reference.0
            || self.neighbourhood_index.0.is_some()
            || self.enable_reanchor.0.is_some()
            || !self.detuned_notes.is_empty()
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for Notifications<T> {
    fn show<A: UiAdaptor<StackType = T>>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero> {
        if let (Some(neighbourhood_index), _) = self.neighbourhood_index {
            (_, adaptor) = adaptor.active_strategy(|strat, _| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("scale ");
                    ui.strong(match strat {
                        StrategyConfig::StaticNeighbourhoods {
                            config:
                                StaticNeighbourhoodsConfig {
                                    scales: neighbourhoods,
                                    ..
                                },
                            ..
                        } => &neighbourhoods[neighbourhood_index % neighbourhoods.len()].name,
                        StrategyConfig::TwoStep {
                            melody:
                                MelodyStrategyConfig::StaticNeighbourhoods(
                                    StaticNeighbourhoodsAsMelodyConfig {
                                        scales: neighbourhoods,
                                        ..
                                    },
                                ),
                            ..
                        } => &neighbourhoods[neighbourhood_index % neighbourhoods.len()].name,
                    });
                });
            });
        }

        if let (Some(enabled), _) = self.enable_reanchor {
            if enabled {
                ui.label("re-setting of the reference on chord match enabled");
            } else {
                ui.label("re-setting of the reference on chord match disabled");
            }
        }

        if let (Some((pattern_index, reference)), _) = &self.chord {
            (_, adaptor) = adaptor.active_strategy(|strat, adaptor| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    match strat {
                        StrategyConfig::TwoStep {
                            harmony:
                                HarmonyStrategyConfig::ChordList(ChordListConfig { patterns, .. }),
                            ..
                        } => {
                            ui.strong(&patterns[*pattern_index % patterns.len()].name);
                            ui.label(" on ");
                            ui.strong(reference.corrected_notename(
                                &NoteNameStyle::Full,
                                adaptor.config().use_cent_values,
                            ));
                        }
                        _ => {}
                    }
                });
            });
        }

        if let (true, _) = &self.reference {
            (_, adaptor) = adaptor.reference(|reference, adaptor| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("reference ");
                    ui.strong(reference.corrected_notename(
                        &NoteNameStyle::Full,
                        adaptor.config().use_cent_values,
                    ));
                });
            });
        }

        for (note, should_be, actual, explanation, _) in &self.detuned_notes {
            (_, adaptor) = adaptor.tuning(*note as usize, |tuning, adaptor| {
                ui.label(format!(
                "note {} not tuned correctly: should be {should_be:.02}, but is {actual:.02}: {explanation}",
                    tuning
                    .stack
                    .corrected_notename(&NoteNameStyle::Full, adaptor.config().use_cent_values,),
            ));
            });
        }

        adaptor
    }
}

impl<T: StackType, A: UiAdaptor<StackType = T>> ReceiveToUiRef<T, A> for Notifications<T> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero> {
        match msg {
            ToUi::UpdateReference {} => {
                self.reference = (true, Instant::now());
            }
            ToUi::SelectScale { index } => {
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
            ToUi::ReanchorOnMatch { reanchor } => {
                self.enable_reanchor = (Some(*reanchor), Instant::now());
            }

            ToUi::CurrentStrategyIndex(_) => {}
            ToUi::Notify { .. } => {} // this will only contain MIDI parse errors (which shouldn't happen?)
            _ => {}
        }

        adaptor
    }
}
