use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use eframe::egui;

use crate::{
    config::{HarmonyStrategyConfig, StrategyConfig},
    gui::r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg::ToUi,
    neighbourhood::CompleteNeighbourhood,
    notename::{HasNoteNames, NoteNameStyle},
    strategy::harmony::{chordlist::ChordListConfig, r#trait::Harmony},
    util::ordered_locks::Zero,
};

pub struct Notifications<T: StackType> {
    harmony: (Option<usize>, Option<Stack<T>>, Instant),
    reference: (bool, Instant),
    scale_index: (Option<usize>, bool, Instant),
    enable_reanchor: (Option<bool>, Instant),
    detuned_notes: VecDeque<(u8, Semitones, Semitones, &'static str, Instant)>,
    cleanup_time: Duration,
}

impl<T: StackType + HasNoteNames> Notifications<T> {
    pub fn new() -> Self {
        Self {
            harmony: (None {}, None {}, Instant::now()),
            reference: (false, Instant::now()),
            scale_index: (None {}, false, Instant::now()),
            enable_reanchor: (None {}, Instant::now()),
            detuned_notes: VecDeque::new(),
            cleanup_time: Duration::from_secs(2),
        }
    }

    pub fn clear_old(&mut self, time: Instant) {
        if time.duration_since(self.reference.1) > self.cleanup_time {
            self.reference.0 = false;
        }

        if let (x, true, old) = self.scale_index {
            if time.duration_since(old) > self.cleanup_time {
                self.scale_index = (x, false, time);
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
        self.harmony.0.is_some()
            || self.reference.0
            || self.scale_index.1
            || self.enable_reanchor.0.is_some()
            || !self.detuned_notes.is_empty()
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for Notifications<T> {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
        if let (Some(scale_index), true, _) = self.scale_index {
            (_, adaptor) = adaptor.scales(|m_scales, _| match m_scales {
                Some(scales) => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label("scale ");
                        ui.strong(&scales[scale_index % scales.len()].name);
                    });
                }
                None {} => {
                    ui.label("no scales for this strategy");
                }
            });
        }

        if let (Some(enabled), _) = self.enable_reanchor {
            if enabled {
                ui.label("re-setting of the reference on chord match enabled");
            } else {
                ui.label("re-setting of the reference on chord match disabled");
            }
        }

        if let (Some(pattern_index), m_reference, _) = &self.harmony {
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
                            if let Some(reference) = m_reference {
                                ui.label(" on ");
                                ui.strong(reference.corrected_notename(
                                    &NoteNameStyle::Full,
                                    adaptor.config().use_cent_values,
                                ));
                            }
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

impl<T: StackType> ReceiveToUiRef<T> for Notifications<T> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        mut adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
        match msg {
            ToUi::UpdateReference {} => {
                self.reference = (true, Instant::now());
            }
            ToUi::SelectScale { index } => {
                self.scale_index = (Some(*index), true, Instant::now());
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
            ToUi::UpdateHarmony {} => {
                (_, adaptor) = adaptor.harmony(|m_harmony, adaptor| match m_harmony {
                    Some(Harmony {
                        pattern_index: Some(pattern_index),
                        reference_key: reference,
                        valid: true,
                        ..
                    }) => {
                        if let Some(scale_index) = self.scale_index.0 {
                            adaptor.scales(|m_scales, adaptor| match m_scales {
                                Some(scales) => {
                                    adaptor.reference(|adaptor_reference, _| {
                                        let reference_stack = scales[scale_index]
                                            .named
                                            .get_absolute_stack(*reference, adaptor_reference);
                                        self.harmony = (
                                            Some(*pattern_index),
                                            Some(reference_stack),
                                            Instant::now(),
                                        )
                                    });
                                }
                                None {} => {
                                    self.harmony = (Some(*pattern_index), None {}, Instant::now());
                                }
                            });
                        } else {
                            self.harmony = (Some(*pattern_index), None {}, Instant::now())
                        }
                    }
                    _ => self.harmony = (None, None, Instant::now()),
                });
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
